//! App-owned session index — isolated from the CLI session browser.
//!
//! Physical transcripts still live under `~/.grok/sessions` (agent store),
//! but the **sidebar only shows what this index registers**. CLI sessions
//! appear only after an explicit import.

use crate::config::{grok_home, AppConfig};
use crate::local::sessions::{
    delete_session, parse_summary_json, scan_sessions, LocalSession,
};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const INDEX_FILE: &str = "sessions_index.json";
const INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionOrigin {
    #[default]
    App,
    CliImport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSessionRecord {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub model: String,
    pub archived: bool,
    #[serde(default)]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub num_messages: u32,
    pub origin: SessionOrigin,
    /// Absolute path to the session directory (under ~/.grok/sessions when known).
    #[serde(default)]
    pub session_path: Option<String>,
}

impl Default for AppSessionRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            cwd: String::new(),
            model: String::new(),
            archived: false,
            archived_at: None,
            created_at: None,
            updated_at: None,
            num_messages: 0,
            origin: SessionOrigin::App,
            session_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexFile {
    version: u32,
    sessions: Vec<AppSessionRecord>,
}

impl Default for IndexFile {
    fn default() -> Self {
        Self {
            version: INDEX_VERSION,
            sessions: Vec::new(),
        }
    }
}

fn index_path() -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?.join(INDEX_FILE))
}

fn load_file() -> IndexFile {
    let Ok(path) = index_path() else {
        return IndexFile::default();
    };
    if !path.is_file() {
        return IndexFile::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => IndexFile::default(),
    }
}

fn save_file(file: &IndexFile) -> Result<()> {
    let path = index_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(file)?;
    std::fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Resolve session dir: recorded path → ~/.grok/sessions/**/id scan.
pub fn resolve_session_dir(rec: &AppSessionRecord) -> Option<PathBuf> {
    if let Some(p) = &rec.session_path {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Some(pb);
        }
    }
    find_cli_session_dir(&rec.id)
}

fn find_cli_session_dir(id: &str) -> Option<PathBuf> {
    find_cli_session_dir_public(id)
}

/// Public lookup of `~/.grok/sessions/**/{id}` for session dir resolution.
pub fn find_cli_session_dir_public(id: &str) -> Option<PathBuf> {
    let home = grok_home()?;
    let root = home.join("sessions");
    if !root.is_dir() {
        return None;
    }
    find_dir_by_id(&root, id)
}

fn find_dir_by_id(dir: &Path, id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for ent in entries.flatten() {
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some(id) {
            if path.join("summary.json").is_file() || path.join("updates.jsonl").is_file() {
                return Some(path);
            }
        }
        if let Some(found) = find_dir_by_id(&path, id) {
            return Some(found);
        }
    }
    None
}

impl AppSessionRecord {
    pub fn to_local(&self) -> LocalSession {
        let path = resolve_session_dir(self).unwrap_or_else(|| {
            PathBuf::from(self.session_path.clone().unwrap_or_else(|| {
                format!("(missing)/{}", self.id)
            }))
        });
        let summary_path = path.join("summary.json");
        LocalSession {
            id: self.id.clone(),
            title: if self.title.trim().is_empty() {
                crate::i18n::t().untitled_paren.into()
            } else {
                self.title.clone()
            },
            cwd: self.cwd.clone(),
            model: self.model.clone(),
            created_at: self.created_at.as_deref().and_then(parse_rfc3339),
            updated_at: self.updated_at.as_deref().and_then(parse_rfc3339),
            num_messages: self.num_messages,
            path,
            summary_path,
        }
    }

    pub fn from_local(s: &LocalSession, origin: SessionOrigin) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            cwd: s.cwd.clone(),
            model: s.model.clone(),
            archived: false,
            archived_at: None,
            created_at: s
                .created_at
                .map(|t| t.to_rfc3339())
                .or_else(|| Some(now.clone())),
            updated_at: s
                .updated_at
                .map(|t| t.to_rfc3339())
                .or_else(|| Some(now)),
            num_messages: s.num_messages,
            origin,
            session_path: Some(s.path.display().to_string()),
        }
    }
}

fn parse_rfc3339(s: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

// ── Public API ──────────────────────────────────────────────────────────

/// Active (non-archived) sessions for the main sidebar, newest first.
pub fn list_active_sessions() -> Vec<LocalSession> {
    let mut recs: Vec<_> = load_file()
        .sessions
        .into_iter()
        .filter(|r| !r.archived)
        .collect();
    recs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    recs.into_iter().map(|r| r.to_local()).collect()
}

/// Archived sessions for the archive panel, newest archive first.
pub fn list_archived_sessions() -> Vec<LocalSession> {
    let mut recs: Vec<_> = load_file()
        .sessions
        .into_iter()
        .filter(|r| r.archived)
        .collect();
    recs.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    recs.into_iter().map(|r| r.to_local()).collect()
}

pub fn index_ids() -> HashSet<String> {
    load_file().sessions.into_iter().map(|r| r.id).collect()
}

pub fn is_registered(id: &str) -> bool {
    load_file().sessions.iter().any(|r| r.id == id)
}

/// Upsert by id (keeps archive flag unless `force_active`).
pub fn upsert_record(mut rec: AppSessionRecord, force_active: bool) -> Result<()> {
    if rec.id.trim().is_empty() {
        bail!("empty session id");
    }
    let mut file = load_file();
    if let Some(existing) = file.sessions.iter_mut().find(|r| r.id == rec.id) {
        // Preserve archive unless forcing active (e.g. user opened it)
        let archived = if force_active {
            false
        } else {
            existing.archived
        };
        let archived_at = if force_active {
            None
        } else {
            existing.archived_at.clone()
        };
        if rec.title.is_empty() {
            rec.title = existing.title.clone();
        }
        if rec.cwd.is_empty() {
            rec.cwd = existing.cwd.clone();
        }
        if rec.model.is_empty() {
            rec.model = existing.model.clone();
        }
        if rec.session_path.is_none() {
            rec.session_path = existing.session_path.clone();
        }
        if rec.num_messages < existing.num_messages {
            rec.num_messages = existing.num_messages;
        }
        rec.archived = archived;
        rec.archived_at = archived_at;
        if rec.created_at.is_none() {
            rec.created_at = existing.created_at.clone();
        }
        *existing = rec;
    } else {
        if force_active {
            rec.archived = false;
            rec.archived_at = None;
        }
        file.sessions.push(rec);
    }
    file.version = INDEX_VERSION;
    save_file(&file)
}

pub fn touch_session(
    id: &str,
    title_hint: Option<&str>,
    cwd: Option<&str>,
    model: Option<&str>,
    bump_messages: bool,
) -> Result<()> {
    let mut file = load_file();
    let now = Utc::now().to_rfc3339();
    if let Some(r) = file.sessions.iter_mut().find(|r| r.id == id) {
        r.updated_at = Some(now);
        if bump_messages {
            r.num_messages = r.num_messages.saturating_add(1);
        }
        if let Some(t) = title_hint {
            let t = t.trim();
            if !t.is_empty()
                && (r.title.is_empty()
                    || r.title == crate::i18n::t().untitled_paren
                    || r.title.starts_with(crate::i18n::t().new_session_title))
            {
                r.title = t.chars().take(48).collect();
            }
        }
        if let Some(c) = cwd {
            if !c.is_empty() {
                r.cwd = c.to_string();
            }
        }
        if let Some(m) = model {
            if !m.is_empty() {
                r.model = m.to_string();
            }
        }
        save_file(&file)?;
    } else {
        // First time we see this id from the agent — register as app session
        let path = find_cli_session_dir(id).map(|p| p.display().to_string());
        let mut title = title_hint.unwrap_or(crate::i18n::t().new_session_title).to_string();
        if title.trim().is_empty() {
            title = crate::i18n::t().new_session_title.into();
        }
        upsert_record(
            AppSessionRecord {
                id: id.to_string(),
                title,
                cwd: cwd.unwrap_or("").to_string(),
                model: model.unwrap_or("").to_string(),
                archived: false,
                archived_at: None,
                created_at: Some(now.clone()),
                updated_at: Some(now),
                num_messages: if bump_messages { 1 } else { 0 },
                origin: SessionOrigin::App,
                session_path: path,
            },
            true,
        )?;
    }
    Ok(())
}

pub fn rename_in_index(id: &str, new_title: &str) -> Result<()> {
    let title = new_title.trim();
    if title.is_empty() {
        bail!("{}", crate::i18n::t().title_empty);
    }
    let mut file = load_file();
    let Some(r) = file.sessions.iter_mut().find(|r| r.id == id) else {
        bail!("{}", crate::i18n::t().not_in_app_index);
    };
    r.title = title.to_string();
    r.updated_at = Some(Utc::now().to_rfc3339());
    save_file(&file)
}

pub fn archive_session(id: &str) -> Result<()> {
    let mut file = load_file();
    let Some(r) = file.sessions.iter_mut().find(|r| r.id == id) else {
        bail!("{}", crate::i18n::t().not_in_app_index);
    };
    r.archived = true;
    r.archived_at = Some(Utc::now().to_rfc3339());
    save_file(&file)
}

pub fn restore_session(id: &str) -> Result<()> {
    let mut file = load_file();
    let Some(r) = file.sessions.iter_mut().find(|r| r.id == id) else {
        bail!("{}", crate::i18n::t().not_in_app_index);
    };
    r.archived = false;
    r.archived_at = None;
    r.updated_at = Some(Utc::now().to_rfc3339());
    save_file(&file)
}

/// Remove from App index. Optionally delete the on-disk CLI session directory.
pub fn delete_from_app(id: &str, delete_disk: bool) -> Result<()> {
    let mut file = load_file();
    let Some(pos) = file.sessions.iter().position(|r| r.id == id) else {
        bail!("{}", crate::i18n::t().not_in_app_index);
    };
    let rec = file.sessions.remove(pos);
    save_file(&file)?;
    if delete_disk {
        if let Some(dir) = resolve_session_dir(&rec) {
            // Best-effort; index already saved
            let _ = delete_session(&dir);
        }
    }
    Ok(())
}

/// Explicit import of a CLI session into the App index.
pub fn import_cli_session(s: &LocalSession) -> Result<()> {
    if s.id.trim().is_empty() {
        bail!("{}", crate::i18n::t().invalid_session_id);
    }
    if is_registered(&s.id) {
        // Already present — unarchive if needed
        restore_session(&s.id)?;
        // Refresh metadata from CLI summary
        let mut rec = AppSessionRecord::from_local(s, SessionOrigin::CliImport);
        rec.origin = SessionOrigin::CliImport;
        upsert_record(rec, true)?;
        return Ok(());
    }
    upsert_record(AppSessionRecord::from_local(s, SessionOrigin::CliImport), true)
}

/// CLI sessions not yet in the App index (for import picker).
pub fn list_cli_import_candidates(limit: usize) -> Vec<LocalSession> {
    let known = index_ids();
    scan_sessions(limit.max(200))
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !known.contains(&s.id))
        .take(limit)
        .collect()
}

/// Refresh title/cwd/model/path from disk summary when available.
pub fn sync_record_from_disk(id: &str) -> Result<()> {
    let mut file = load_file();
    let Some(r) = file.sessions.iter_mut().find(|r| r.id == id) else {
        return Ok(());
    };
    let Some(dir) = resolve_session_dir(r) else {
        return Ok(());
    };
    let summary = dir.join("summary.json");
    if !summary.is_file() {
        r.session_path = Some(dir.display().to_string());
        save_file(&file)?;
        return Ok(());
    }
    let text = std::fs::read_to_string(&summary)?;
    if let Some(local) = parse_summary_json(&text, &summary, &dir) {
        if !local.title.is_empty() && local.title != crate::i18n::t().untitled_paren {
            r.title = local.title;
        }
        if !local.cwd.is_empty() {
            r.cwd = local.cwd;
        }
        if !local.model.is_empty() {
            r.model = local.model;
        }
        if local.num_messages > r.num_messages {
            r.num_messages = local.num_messages;
        }
        if let Some(u) = local.updated_at {
            r.updated_at = Some(u.to_rfc3339());
        }
    }
    r.session_path = Some(dir.display().to_string());
    save_file(&file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_roundtrip_fields() {
        let rec = AppSessionRecord {
            id: "abc".into(),
            title: "hello".into(),
            cwd: r"D:\proj".into(),
            model: "grok-4.5".into(),
            archived: false,
            archived_at: None,
            created_at: Some("2026-01-01T00:00:00Z".into()),
            updated_at: Some("2026-01-02T00:00:00Z".into()),
            num_messages: 3,
            origin: SessionOrigin::App,
            session_path: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back: AppSessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "abc");
        assert_eq!(back.origin, SessionOrigin::App);
        assert!(!back.archived);
    }
}
