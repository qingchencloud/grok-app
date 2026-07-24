//! Scan ~/.grok/sessions for past conversations (same store as CLI TUI / ACP).

use crate::acp::TimelineItem;
use crate::config::grok_home;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LocalSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub model: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub num_messages: u32,
    pub path: PathBuf,
    pub summary_path: PathBuf,
}

/// One project bucket in the sidebar (grouped by session cwd).
#[derive(Debug, Clone)]
pub struct ProjectGroup {
    /// Normalized absolute/cwd path used as stable key.
    pub key: String,
    /// Short display name (last path segment).
    pub name: String,
    /// Full path for tooltip.
    pub path_display: String,
    pub sessions: Vec<LocalSession>,
}

#[derive(Debug, Deserialize)]
struct SummaryFile {
    #[serde(default)]
    info: SummaryInfo,
    #[serde(default)]
    session_summary: Option<String>,
    #[serde(default)]
    generated_title: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    last_active_at: Option<String>,
    #[serde(default)]
    num_messages: Option<u32>,
    #[serde(default)]
    num_chat_messages: Option<u32>,
    #[serde(default)]
    current_model_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SummaryInfo {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
}

/// Walk ~/.grok/sessions/**/summary.json and return newest first.
pub fn scan_sessions(limit: usize) -> Result<Vec<LocalSession>> {
    let home = match grok_home() {
        Some(h) => h,
        None => return Ok(Vec::new()),
    };
    let root = home.join("sessions");
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    scan_dir(&root, &mut out)?;
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if out.len() > limit {
        out.truncate(limit);
    }
    Ok(out)
}

fn scan_dir(dir: &Path, out: &mut Vec<LocalSession>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.is_dir() {
            let summary = path.join("summary.json");
            if summary.is_file() {
                if let Some(s) = parse_summary(&summary, &path) {
                    out.push(s);
                }
            } else {
                scan_dir(&path, out)?;
            }
        }
    }
    Ok(())
}

fn parse_summary(summary_path: &Path, session_dir: &Path) -> Option<LocalSession> {
    let text = std::fs::read_to_string(summary_path).ok()?;
    parse_summary_json(&text, summary_path, session_dir)
}

/// Parse a `summary.json` body (shipped path for unit tests + scan).
pub fn parse_summary_json(
    text: &str,
    summary_path: &Path,
    session_dir: &Path,
) -> Option<LocalSession> {
    let s: SummaryFile = serde_json::from_str(text).ok()?;
    let id = s
        .info
        .id
        .clone()
        .or_else(|| {
            session_dir
                .file_name()
                .and_then(|n| n.to_str().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "unknown".into());
    let title = s
        .generated_title
        .or(s.session_summary)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| crate::i18n::t().untitled_paren.into());
    let cwd = s.info.cwd.unwrap_or_default();
    let model = s.current_model_id.unwrap_or_default();
    let created_at = parse_ts(s.created_at.as_deref());
    let updated_at = parse_ts(s.last_active_at.as_deref().or(s.updated_at.as_deref()));
    let num_messages = s.num_chat_messages.or(s.num_messages).unwrap_or(0);

    Some(LocalSession {
        id,
        title,
        cwd,
        model,
        created_at,
        updated_at,
        num_messages,
        path: session_dir.to_path_buf(),
        summary_path: summary_path.to_path_buf(),
    })
}

fn parse_ts(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ")
                .ok()
                .map(|n| n.and_utc())
        })
}

/// Normalize a cwd for grouping (trim, unify separators, strip trailing slash).
pub fn normalize_project_key(cwd: &str) -> String {
    let t = cwd.trim();
    if t.is_empty() {
        return String::new();
    }
    let mut s = t.replace('/', "\\");
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    // Drive letters: C:\foo
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let mut chars = s.chars();
        let d = chars.next().unwrap().to_ascii_uppercase();
        s = format!("{d}{}", chars.collect::<String>());
    }
    s
}

/// Short project name from cwd (last folder, or drive root).
pub fn project_display_name(cwd: &str) -> String {
    let key = normalize_project_key(cwd);
    if key.is_empty() {
        return crate::i18n::t().no_project.into();
    }
    let p = Path::new(&key);
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    key
}

/// Rename a session by rewriting `generated_title` / `session_summary` in summary.json.
pub fn rename_session(summary_path: &Path, new_title: &str) -> Result<()> {
    let title = new_title.trim();
    if title.is_empty() {
        bail!("{}", crate::i18n::t().title_empty);
    }
    let text = std::fs::read_to_string(summary_path)
        .with_context(|| format!("read {}", summary_path.display()))?;
    let mut v: serde_json::Value = serde_json::from_str(&text).context("parse summary.json")?;
    v["generated_title"] = serde_json::Value::String(title.to_string());
    v["session_summary"] = serde_json::Value::String(title.to_string());
    let out = serde_json::to_string_pretty(&v)?;
    std::fs::write(summary_path, out)
        .with_context(|| format!("write {}", summary_path.display()))?;
    Ok(())
}

/// Delete a session directory under ~/.grok/sessions (recursive).
pub fn delete_session(session_dir: &Path) -> Result<()> {
    let Some(home) = grok_home() else {
        bail!("no GROK_HOME");
    };
    let root = home.join("sessions");
    let root = root.canonicalize().unwrap_or(root);
    let dir = session_dir
        .canonicalize()
        .with_context(|| format!("resolve {}", session_dir.display()))?;
    if !dir.starts_with(&root) {
        bail!("拒绝删除 sessions 目录外的路径: {}", dir.display());
    }
    if !dir.is_dir() {
        bail!("会话目录不存在");
    }
    std::fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
    Ok(())
}

/// Group sessions by project directory. Groups ordered by newest session first;
/// sessions inside each group newest first. Optional `prefer_cwd` is sorted to top.
pub fn group_sessions_by_project(
    sessions: &[LocalSession],
    prefer_cwd: Option<&str>,
) -> Vec<ProjectGroup> {
    let mut map: BTreeMap<String, Vec<LocalSession>> = BTreeMap::new();
    for s in sessions {
        let key = normalize_project_key(&s.cwd);
        map.entry(key).or_default().push(s.clone());
    }

    let prefer = prefer_cwd.map(normalize_project_key).unwrap_or_default();

    let mut groups: Vec<ProjectGroup> = map
        .into_iter()
        .map(|(key, mut sessions)| {
            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let path_display = if key.is_empty() {
                crate::i18n::t().unbound_cwd.into()
            } else {
                key.clone()
            };
            let name = if key.is_empty() {
                crate::i18n::t().no_project.into()
            } else {
                project_display_name(&key)
            };
            ProjectGroup {
                key,
                name,
                path_display,
                sessions,
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        // preferred project first
        let a_pref = !prefer.is_empty() && a.key.eq_ignore_ascii_case(&prefer);
        let b_pref = !prefer.is_empty() && b.key.eq_ignore_ascii_case(&prefer);
        match (a_pref, b_pref) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let at = a.sessions.first().and_then(|s| s.updated_at);
                let bt = b.sessions.first().and_then(|s| s.updated_at);
                bt.cmp(&at).then_with(|| a.name.cmp(&b.name))
            }
        }
    });
    groups
}

/// Best-effort reconstruct a simple timeline from updates.jsonl for preview / resume UI.
pub fn load_session_timeline(session_dir: &Path, max_items: usize) -> Vec<TimelineItem> {
    let path = session_dir.join("updates.jsonl");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            debug!("read updates.jsonl: {e}");
            return Vec::new();
        }
    };
    parse_updates_jsonl(&text, max_items)
}

/// Parse ACP `updates.jsonl` body into UI timeline items (testable pure path).
///
/// Processes the **entire** file, then keeps the last `max_items` entries so long
/// sessions still show the recent conversation (not only the first N events).
pub fn parse_updates_jsonl(text: &str, max_items: usize) -> Vec<TimelineItem> {
    let mut items: Vec<TimelineItem> = Vec::new();
    let mut cur_user: Option<String> = None;
    let mut cur_assistant: Option<String> = None;
    let mut cur_thought: Option<String> = None;

    let flush_user = |items: &mut Vec<TimelineItem>, buf: &mut Option<String>| {
        if let Some(t) = buf.take() {
            if !t.is_empty() {
                items.push(TimelineItem::UserMessage {
                    id: Uuid::new_v4().to_string(),
                    text: t,
                    attachments: Vec::new(),
                });
            }
        }
    };
    let flush_assistant = |items: &mut Vec<TimelineItem>, buf: &mut Option<String>| {
        if let Some(t) = buf.take() {
            if !t.is_empty() {
                items.push(TimelineItem::AssistantMessage {
                    id: Uuid::new_v4().to_string(),
                    text: t,
                    streaming: false,
                });
            }
        }
    };
    let flush_thought = |items: &mut Vec<TimelineItem>, buf: &mut Option<String>| {
        if let Some(t) = buf.take() {
            if !t.is_empty() {
                // Cap very long thoughts in history so the UI stays usable
                let text = if t.chars().count() > 4000 {
                    let head: String = t.chars().take(4000).collect();
                    format!("{}\n{}", head, crate::i18n::t().thought_truncated)
                } else {
                    t
                };
                items.push(TimelineItem::Thought {
                    id: Uuid::new_v4().to_string(),
                    text,
                });
            }
        }
    };

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Accept both session/update and _x.ai/session/update envelopes
        let update = v
            .pointer("/params/update")
            .cloned()
            .or_else(|| v.get("update").cloned())
            .unwrap_or(serde_json::Value::Null);
        let kind = update
            .get("sessionUpdate")
            .and_then(|x| x.as_str())
            .unwrap_or("");

        match kind {
            "user_message_chunk" => {
                flush_assistant(&mut items, &mut cur_assistant);
                flush_thought(&mut items, &mut cur_thought);
                let chunk = extract_chunk_text(&update);
                if !chunk.is_empty() {
                    cur_user.get_or_insert_with(String::new).push_str(&chunk);
                }
            }
            "agent_message_chunk" => {
                flush_user(&mut items, &mut cur_user);
                flush_thought(&mut items, &mut cur_thought);
                let chunk = extract_chunk_text(&update);
                if !chunk.is_empty() {
                    cur_assistant
                        .get_or_insert_with(String::new)
                        .push_str(&chunk);
                }
            }
            "agent_thought_chunk" => {
                flush_user(&mut items, &mut cur_user);
                let chunk = extract_chunk_text(&update);
                if !chunk.is_empty() {
                    cur_thought.get_or_insert_with(String::new).push_str(&chunk);
                }
            }
            "tool_call" => {
                flush_user(&mut items, &mut cur_user);
                flush_thought(&mut items, &mut cur_thought);
                flush_assistant(&mut items, &mut cur_assistant);
                let id = update
                    .get("toolCallId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = update
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let kind = update
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .or_else(|| {
                        update
                            .pointer("/_meta/x.ai/tool/kind")
                            .and_then(|x| x.as_str())
                    })
                    .or_else(|| {
                        // updateParams.kind in outer _meta
                        v.pointer("/params/_meta/updateParams/kind")
                            .and_then(|x| x.as_str())
                    })
                    .unwrap_or("other")
                    .to_string();
                let detail = update
                    .get("rawInput")
                    .map(|r| serde_json::to_string_pretty(r).unwrap_or_default())
                    .unwrap_or_default();
                items.push(TimelineItem::Tool {
                    id,
                    title,
                    kind,
                    status: "pending".into(),
                    detail,
                });
            }
            "tool_call_update" => {
                let id = update
                    .get("toolCallId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                if let Some(TimelineItem::Tool {
                    status,
                    title,
                    detail,
                    ..
                }) = items
                    .iter_mut()
                    .rev()
                    .find(|i| matches!(i, TimelineItem::Tool { id: tid, .. } if tid == id))
                {
                    if let Some(st) = update.get("status").and_then(|x| x.as_str()) {
                        *status = st.to_string();
                    }
                    if let Some(tt) = update.get("title").and_then(|x| x.as_str()) {
                        if !tt.is_empty() {
                            *title = tt.to_string();
                        }
                    }
                    // Append text content if present
                    if let Some(arr) = update.get("content").and_then(|c| c.as_array()) {
                        for item in arr {
                            if let Some(t) = item.pointer("/content/text").and_then(|t| t.as_str())
                            {
                                if !t.is_empty() {
                                    if !detail.is_empty() {
                                        detail.push('\n');
                                    }
                                    detail.push_str(t);
                                }
                            }
                        }
                    }
                }
            }
            "plan" => {
                flush_user(&mut items, &mut cur_user);
                flush_thought(&mut items, &mut cur_thought);
                flush_assistant(&mut items, &mut cur_assistant);
                // ignore detailed plan in history for now — optional
            }
            _ => {}
        }
    }

    flush_user(&mut items, &mut cur_user);
    flush_thought(&mut items, &mut cur_thought);
    flush_assistant(&mut items, &mut cur_assistant);

    // History UX: drop completed tool noise so user/assistant text stays visible.
    // Keep failed tools; collapse long successful tool runs.
    items = compact_history_tools(items);

    // Keep only last max_items for UI (recent conversation)
    if max_items > 0 && items.len() > max_items {
        items = items.split_off(items.len() - max_items);
    }
    items
}

/// Collapse consecutive completed tools into a single status line; strip tool detail.
fn compact_history_tools(items: Vec<TimelineItem>) -> Vec<TimelineItem> {
    let mut out: Vec<TimelineItem> = Vec::with_capacity(items.len());
    let mut pending_ok_tools: Vec<String> = Vec::new();

    let flush_tools = |out: &mut Vec<TimelineItem>, titles: &mut Vec<String>| {
        if titles.is_empty() {
            return;
        }
        if titles.len() == 1 {
            out.push(TimelineItem::Tool {
                id: Uuid::new_v4().to_string(),
                title: titles[0].clone(),
                kind: String::new(),
                status: "completed".into(),
                detail: String::new(),
            });
        } else {
            out.push(TimelineItem::Status {
                id: Uuid::new_v4().to_string(),
                text: crate::i18n::tools_summary_line(titles.len(), &titles.join(" · ")),
            });
        }
        titles.clear();
    };

    for item in items {
        match item {
            TimelineItem::Tool {
                title,
                status,
                detail: _,
                kind,
                id,
            } => {
                let st = status.to_ascii_lowercase();
                if st == "completed" || st == "success" {
                    pending_ok_tools.push(if title.is_empty() {
                        "tool".into()
                    } else {
                        title
                    });
                } else {
                    flush_tools(&mut out, &mut pending_ok_tools);
                    // Keep failed / pending tools but without huge detail
                    out.push(TimelineItem::Tool {
                        id,
                        title,
                        kind,
                        status,
                        detail: String::new(),
                    });
                }
            }
            other => {
                flush_tools(&mut out, &mut pending_ok_tools);
                // Cap thought text for history
                match other {
                    TimelineItem::Thought { id, text } => {
                        let text = if text.chars().count() > 500 {
                            let head: String = text.chars().take(500).collect();
                            format!("{head}…")
                        } else {
                            text
                        };
                        out.push(TimelineItem::Thought { id, text });
                    }
                    x => out.push(x),
                }
            }
        }
    }
    flush_tools(&mut out, &mut pending_ok_tools);
    out
}

fn extract_chunk_text(update: &serde_json::Value) -> String {
    if let Some(t) = update.pointer("/content/text").and_then(|t| t.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update.get("content").and_then(|c| c.as_str()) {
        return t.to_string();
    }
    // content as array of parts
    if let Some(arr) = update.get("content").and_then(|c| c.as_array()) {
        let mut s = String::new();
        for part in arr {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                s.push_str(t);
            } else if let Some(t) = part.pointer("/content/text").and_then(|t| t.as_str()) {
                s.push_str(t);
            }
        }
        return s;
    }
    String::new()
}
