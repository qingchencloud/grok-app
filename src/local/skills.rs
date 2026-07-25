//! Discover and manage Grok skills (`SKILL.md` packages).
//!
//! Prefer `grok inspect --json` (same list as the CLI). Fall back to filesystem
//! scan of known skill roots when the CLI is unavailable.

use crate::config::{grok_home, resolve_grok_binary};
use crate::spawn_util;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, DocumentMut, Item};

/// One skill as shown in Settings → Skills.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    /// `user` | `project` | `bundled` | `claude` | `cursor` | `agents` | …
    pub source: String,
    pub path: PathBuf,
    pub user_invocable: bool,
    /// Present in `[skills].disabled` in `~/.grok/config.toml`.
    pub disabled: bool,
    /// Bundled / read-only path — cannot delete; disable still allowed.
    pub readonly: bool,
}

#[derive(Debug, Deserialize)]
struct InspectRoot {
    #[serde(default)]
    skills: Vec<InspectSkill>,
}

#[derive(Debug, Deserialize)]
struct InspectSkill {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    source: InspectSource,
    #[serde(default, rename = "userInvocable", alias = "user_invocable")]
    user_invocable: bool,
}

#[derive(Debug, Default, Deserialize)]
struct InspectSource {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    path: String,
}

/// List skills for `cwd` using the configured `grok` binary when possible.
pub fn list_skills(configured_grok: &str, cwd: &str) -> Vec<SkillEntry> {
    let disabled = load_disabled_set();
    let mut list = match try_inspect(configured_grok, cwd) {
        Ok(v) if !v.is_empty() => v,
        Ok(_) | Err(_) => scan_filesystem(cwd),
    };
    for s in &mut list {
        s.disabled = disabled.contains(&s.name);
        s.readonly = is_readonly_source(&s.source, &s.path);
    }
    list.sort_by(|a, b| {
        a.source.cmp(&b.source).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });
    list
}

fn try_inspect(configured_grok: &str, cwd: &str) -> Result<Vec<SkillEntry>> {
    let bin = resolve_grok_binary(configured_grok)?;
    let mut cmd = spawn_util::command_piped(&bin);
    cmd.args(["inspect", "--json"]);
    let cwd_trim = cwd.trim();
    if !cwd_trim.is_empty() {
        let p = Path::new(cwd_trim);
        if p.is_dir() {
            cmd.current_dir(p);
        }
    }
    let out = cmd
        .output()
        .with_context(|| format!("run {} inspect --json", bin.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("inspect failed: {err}");
    }
    let root: InspectRoot = serde_json::from_slice(&out.stdout).context("parse inspect JSON")?;
    Ok(root
        .skills
        .into_iter()
        .map(|s| {
            let path = PathBuf::from(&s.source.path);
            let source = if s.source.kind.is_empty() {
                infer_source_label(&path)
            } else {
                s.source.kind
            };
            SkillEntry {
                name: s.name,
                description: s.description,
                source,
                path,
                user_invocable: s.user_invocable,
                disabled: false,
                readonly: false,
            }
        })
        .collect())
}

/// Filesystem fallback when CLI inspect is unavailable.
pub fn scan_filesystem(cwd: &str) -> Vec<SkillEntry> {
    let mut by_name: BTreeMap<String, SkillEntry> = BTreeMap::new();
    // Higher priority inserted later wins (matches Grok: project > user > bundled-ish).
    // We insert low priority first so later overwrites.
    let roots = skill_scan_roots(cwd);
    // Sort so higher priority overwrites: (priority, path)
    let mut ordered = roots;
    ordered.sort_by_key(|(prio, _)| *prio);
    for (_prio, root) in ordered {
        if !root.is_dir() {
            continue;
        }
        walk_skill_root(&root, &mut by_name);
    }
    by_name.into_values().collect()
}

/// (priority ascending = lower first; higher last wins on name collision)
fn skill_scan_roots(cwd: &str) -> Vec<(u8, PathBuf)> {
    let mut roots = Vec::new();
    if let Some(home) = grok_home() {
        roots.push((10, home.join("bundled").join("skills")));
        roots.push((20, home.join("skills")));
    }
    if let Some(home) = dirs::home_dir() {
        roots.push((15, home.join(".agents").join("skills")));
        roots.push((12, home.join(".claude").join("skills")));
        roots.push((11, home.join(".cursor").join("skills")));
    }
    let cwd = cwd.trim();
    if !cwd.is_empty() {
        let c = PathBuf::from(cwd);
        // Walk from cwd up a few parents for project .grok/skills
        let mut cur = Some(c.as_path());
        let mut depth = 0;
        while let Some(p) = cur {
            if depth > 8 {
                break;
            }
            roots.push((50 + depth, p.join(".grok").join("skills")));
            roots.push((48 + depth, p.join(".agents").join("skills")));
            roots.push((46 + depth, p.join(".claude").join("skills")));
            roots.push((44 + depth, p.join(".cursor").join("skills")));
            if p.join(".git").exists() {
                // repo root already covered; stop climbing further for project skills
                break;
            }
            cur = p.parent();
            depth += 1;
        }
    }
    roots
}

fn walk_skill_root(root: &Path, out: &mut BTreeMap<String, SkillEntry>) {
    let source = infer_source_label(root);
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.file_name().and_then(|s| s.to_str()) == Some("_src") {
            continue;
        }
        // skill dir with SKILL.md
        let skill_md = if path.is_dir() {
            let md = path.join("SKILL.md");
            if md.is_file() {
                md
            } else {
                // also accept skill.md
                let alt = path.join("skill.md");
                if alt.is_file() {
                    alt
                } else {
                    continue;
                }
            }
        } else if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("SKILL.md"))
        {
            path.clone()
        } else {
            continue;
        };

        if let Some(entry) = parse_skill_file(&skill_md, &source) {
            out.insert(entry.name.clone(), entry);
        }
    }
}

fn parse_skill_file(skill_md: &Path, source: &str) -> Option<SkillEntry> {
    let text = std::fs::read_to_string(skill_md).ok()?;
    let (name_fm, desc_fm) = parse_frontmatter(&text);
    let dir_name = skill_md
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("skill")
        .to_string();
    let name = name_fm
        .filter(|s| !s.is_empty())
        .unwrap_or(dir_name)
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let description = desc_fm.unwrap_or_else(|| first_body_paragraph(&text));
    Some(SkillEntry {
        name,
        description,
        source: source.to_string(),
        path: skill_md.to_path_buf(),
        user_invocable: true,
        disabled: false,
        readonly: false,
    })
}

/// Minimal YAML-ish frontmatter: `name:` / `description:` (multi-line `>` supported simply).
pub fn parse_frontmatter(text: &str) -> (Option<String>, Option<String>) {
    let t = text.trim_start_matches('\u{feff}');
    if !t.starts_with("---") {
        return (None, None);
    }
    let Some(rest) = t.strip_prefix("---") else {
        return (None, None);
    };
    let rest = rest.trim_start_matches(['\r', '\n']);
    let Some(end) = rest.find("\n---") else {
        return (None, None);
    };
    let block = &rest[..end];
    let mut name = None;
    let mut desc = None;
    let mut lines = block.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(unquote(v.trim()));
        } else if let Some(v) = line.strip_prefix("description:") {
            let v = v.trim();
            if v == ">" || v == "|" {
                let mut parts = Vec::new();
                while let Some(n) = lines.peek() {
                    if n.starts_with(' ') || n.starts_with('\t') || n.trim().is_empty() {
                        let n = lines.next().unwrap();
                        parts.push(n.trim());
                    } else if n.contains(':') && !n.starts_with(' ') {
                        break;
                    } else {
                        break;
                    }
                }
                desc = Some(
                    parts
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            } else {
                desc = Some(unquote(v));
            }
        }
    }
    (name, desc)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn first_body_paragraph(text: &str) -> String {
    let t = text.trim_start_matches('\u{feff}');
    let body = if t.starts_with("---") {
        t.strip_prefix("---")
            .and_then(|r| {
                let r = r.trim_start_matches(['\r', '\n']);
                r.find("\n---").map(|i| r[i + 4..].trim_start())
            })
            .unwrap_or(t)
    } else {
        t
    };
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

fn infer_source_label(path: &Path) -> String {
    let s = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if s.contains("/bundled/skills") {
        "bundled".into()
    } else if s.contains("/.claude/") {
        "claude".into()
    } else if s.contains("/.cursor/") {
        "cursor".into()
    } else if s.contains("/.agents/") {
        "agents".into()
    } else if s.contains("/.grok/skills") {
        // user home vs project: crude check
        if let Some(home) = grok_home() {
            if path.starts_with(home.join("skills")) {
                return "user".into();
            }
        }
        "project".into()
    } else {
        "other".into()
    }
}

fn is_readonly_source(source: &str, path: &Path) -> bool {
    source == "bundled"
        || path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
            .contains("/bundled/skills")
}

// ── config.toml [skills].disabled ──────────────────────────────────────

pub fn load_disabled_set() -> BTreeSet<String> {
    let Some(path) = CliSkillsConfig::config_path() else {
        return BTreeSet::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    parse_disabled_from_toml(&text)
}

pub fn parse_disabled_from_toml(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let Ok(v) = text.parse::<toml::Value>() else {
        return set;
    };
    let Some(arr) = v
        .get("skills")
        .and_then(|t| t.get("disabled"))
        .and_then(|x| x.as_array())
    else {
        return set;
    };
    for item in arr {
        if let Some(s) = item.as_str() {
            let s = s.trim();
            if !s.is_empty() {
                set.insert(s.to_string());
            }
        }
    }
    set
}

/// Enable or disable a skill by name in `~/.grok/config.toml` `[skills].disabled`.
pub fn set_skill_disabled(name: &str, disabled: bool) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("empty skill name");
    }
    let path = CliSkillsConfig::config_path().context("no GROK_HOME")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = if path.is_file() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };
    let out = merge_disabled(&existing, name, disabled)?;
    std::fs::write(&path, out)?;
    Ok(())
}

/// Pure merge helper (testable).
pub fn merge_disabled(existing: &str, name: &str, disabled: bool) -> Result<String> {
    let mut doc = if existing.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing
            .parse::<DocumentMut>()
            .context("parse config.toml")?
    };
    if !doc.as_table().contains_key("skills") || !doc["skills"].is_table() {
        doc["skills"] = toml_edit::table();
    }
    let skills = doc["skills"].as_table_mut().context("skills table")?;
    let mut set: BTreeSet<String> = BTreeSet::new();
    if let Some(Item::Value(v)) = skills.get("disabled") {
        if let Some(arr) = v.as_array() {
            for item in arr.iter() {
                if let Some(s) = item.as_str() {
                    let s = s.trim();
                    if !s.is_empty() {
                        set.insert(s.to_string());
                    }
                }
            }
        }
    }
    if disabled {
        set.insert(name.to_string());
    } else {
        set.remove(name);
    }
    let mut arr = Array::new();
    for s in &set {
        arr.push(s.as_str());
    }
    skills["disabled"] = value(arr);
    // Keep empty paths/ignore if missing so section is recognizable
    if !skills.contains_key("paths") {
        skills["paths"] = value(Array::new());
    }
    Ok(doc.to_string())
}

struct CliSkillsConfig;
impl CliSkillsConfig {
    fn config_path() -> Option<PathBuf> {
        grok_home().map(|h| h.join("config.toml"))
    }
}

// ── open helpers ───────────────────────────────────────────────────────

/// Open SKILL.md (or its parent folder) in the OS file manager / default app.
pub fn open_skill_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("missing {}", path.display());
    }
    open_path(path)
}

pub fn open_skill_folder(path: &Path) -> Result<()> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf())
    };
    if !dir.exists() {
        bail!("missing {}", dir.display());
    }
    open_path(&dir)
}

fn open_path(path: &Path) -> Result<()> {
    let s = path.to_string_lossy();
    #[cfg(windows)]
    {
        // explorer for dirs; ShellExecute "open" for files
        if path.is_dir() {
            let _ = spawn_util::command("explorer")
                .arg(path.as_os_str())
                .spawn()
                .context("explorer")?;
        } else {
            spawn_util::open_url(&s);
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(path)
            .spawn()
            .context("open")?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .context("xdg-open")?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        let _ = s;
        bail!("open not supported on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_name_desc() {
        let t = "---\nname: commit\ndescription: Make commits.\n---\n\n# Hi\n";
        let (n, d) = parse_frontmatter(t);
        assert_eq!(n.as_deref(), Some("commit"));
        assert_eq!(d.as_deref(), Some("Make commits."));
    }

    #[test]
    fn frontmatter_multiline_desc() {
        let t = "---\nname: x\ndescription: >\n  line one\n  line two\n---\nbody\n";
        let (n, d) = parse_frontmatter(t);
        assert_eq!(n.as_deref(), Some("x"));
        assert!(d.as_ref().unwrap().contains("line one"));
    }

    #[test]
    fn merge_disabled_add_remove() {
        let base = "[skills]\ndisabled = [\"a\"]\n";
        let out = merge_disabled(base, "b", true).unwrap();
        let set = parse_disabled_from_toml(&out);
        assert!(set.contains("a"));
        assert!(set.contains("b"));
        let out2 = merge_disabled(&out, "a", false).unwrap();
        let set2 = parse_disabled_from_toml(&out2);
        assert!(!set2.contains("a"));
        assert!(set2.contains("b"));
    }
}
