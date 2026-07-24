//! Read/write ~/.grok/config.toml — maps slash-command style settings into editable fields.

use crate::config::grok_home;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toml_edit::{value, DocumentMut};

/// Subset of CLI config that the desktop settings UI exposes
/// (corresponds to common `/settings` + slash toggles).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CliTomlConfig {
    // [cli]
    pub auto_update: bool,
    // [models]
    pub default_model: String,
    // [ui]
    pub yolo: bool,
    pub permission_mode: String,
    pub compact_mode: bool,
    pub vim_mode: bool,
    pub show_thinking_blocks: bool,
    pub simple_mode: bool,
    pub screen_mode: String,
    pub page_flip_on_send: bool,
    pub remember_tool_approvals: bool,
    pub group_tool_verbs: bool,
    pub max_thoughts_width: i64,
    // [session]
    pub auto_compact_threshold_percent: i64,
    pub load_envrc: bool,
    // [features]
    pub telemetry: bool,
    pub feedback: bool,
    pub lsp_tools: bool,
    pub codebase_indexing: bool,
    pub remote_fetch: bool,
    // [tools]
    pub respect_gitignore: bool,
    // raw path
    pub path: Option<PathBuf>,
    pub loaded: bool,
}

impl Default for CliTomlConfig {
    fn default() -> Self {
        Self {
            auto_update: true,
            default_model: "grok-4.5".into(),
            yolo: false,
            permission_mode: "default".into(),
            compact_mode: false,
            vim_mode: false,
            show_thinking_blocks: true,
            simple_mode: true,
            screen_mode: "fullscreen".into(),
            page_flip_on_send: true,
            remember_tool_approvals: false,
            group_tool_verbs: true,
            max_thoughts_width: 120,
            auto_compact_threshold_percent: 85,
            load_envrc: true,
            telemetry: false,
            feedback: true,
            lsp_tools: false,
            codebase_indexing: true,
            remote_fetch: true,
            respect_gitignore: false,
            path: None,
            loaded: false,
        }
    }
}

impl CliTomlConfig {
    pub fn config_path() -> Option<PathBuf> {
        grok_home().map(|h| h.join("config.toml"))
    }

    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        if !path.is_file() {
            let mut c = Self::default();
            c.path = Some(path);
            return c;
        }
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let mut c = parse_toml(&s);
                c.path = Some(path);
                c.loaded = true;
                c
            }
            Err(_) => {
                let mut c = Self::default();
                c.path = Some(path);
                c
            }
        }
    }

    /// Write known keys back into config.toml, preserving unrelated keys/comments.
    pub fn save(&self) -> Result<()> {
        let path = self
            .path
            .clone()
            .or_else(Self::config_path)
            .context("no GROK_HOME")?;
        self.save_to_path(&path)
    }

    /// Merge this config into `path` without dropping unknown tables/keys.
    pub fn save_to_path(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existing = if path.is_file() {
            std::fs::read_to_string(path).unwrap_or_default()
        } else {
            String::new()
        };
        let out = merge_config_toml(&existing, self);
        std::fs::write(path, out)?;
        Ok(())
    }

    /// Load from an arbitrary TOML string (unit tests).
    pub fn from_toml_str(s: &str) -> Self {
        parse_toml(s)
    }
}

/// Pure merge of desktop-managed keys into an existing TOML document body.
pub fn merge_config_toml(existing: &str, cfg: &CliTomlConfig) -> String {
    let mut doc = if existing.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing
            .parse::<DocumentMut>()
            .unwrap_or_else(|_| DocumentMut::new())
    };

    ensure_table(&mut doc, "cli");
    doc["cli"]["auto_update"] = value(cfg.auto_update);

    ensure_table(&mut doc, "models");
    if !cfg.default_model.trim().is_empty() {
        doc["models"]["default"] = value(cfg.default_model.as_str());
    }

    ensure_table(&mut doc, "ui");
    doc["ui"]["yolo"] = value(cfg.yolo);
    doc["ui"]["permission_mode"] = value(cfg.permission_mode.as_str());
    doc["ui"]["compact_mode"] = value(cfg.compact_mode);
    doc["ui"]["vim_mode"] = value(cfg.vim_mode);
    doc["ui"]["show_thinking_blocks"] = value(cfg.show_thinking_blocks);
    doc["ui"]["simple_mode"] = value(cfg.simple_mode);
    doc["ui"]["screen_mode"] = value(cfg.screen_mode.as_str());
    doc["ui"]["page_flip_on_send"] = value(cfg.page_flip_on_send);
    doc["ui"]["remember_tool_approvals"] = value(cfg.remember_tool_approvals);
    doc["ui"]["group_tool_verbs"] = value(cfg.group_tool_verbs);
    doc["ui"]["max_thoughts_width"] = value(cfg.max_thoughts_width);

    ensure_table(&mut doc, "session");
    doc["session"]["auto_compact_threshold_percent"] =
        value(cfg.auto_compact_threshold_percent);
    doc["session"]["load_envrc"] = value(cfg.load_envrc);

    ensure_table(&mut doc, "features");
    doc["features"]["telemetry"] = value(cfg.telemetry);
    doc["features"]["feedback"] = value(cfg.feedback);
    doc["features"]["lsp_tools"] = value(cfg.lsp_tools);
    doc["features"]["codebase_indexing"] = value(cfg.codebase_indexing);
    doc["features"]["remote_fetch"] = value(cfg.remote_fetch);

    ensure_table(&mut doc, "tools");
    doc["tools"]["respect_gitignore"] = value(cfg.respect_gitignore);

    doc.to_string()
}

fn ensure_table(doc: &mut DocumentMut, name: &str) {
    if !doc.as_table().contains_key(name) {
        doc[name] = toml_edit::table();
    } else if !doc[name].is_table() {
        doc[name] = toml_edit::table();
    }
}

pub fn parse_toml(s: &str) -> CliTomlConfig {
    let mut c = CliTomlConfig::default();
    let v: toml::Value = match toml::from_str(s) {
        Ok(v) => v,
        Err(_) => return c,
    };

    if let Some(t) = v.get("cli") {
        c.auto_update = t.get("auto_update").and_then(|x| x.as_bool()).unwrap_or(c.auto_update);
    }
    if let Some(t) = v.get("models") {
        if let Some(m) = t.get("default").and_then(|x| x.as_str()) {
            c.default_model = m.to_string();
        }
    }
    if let Some(t) = v.get("ui") {
        c.yolo = t.get("yolo").and_then(|x| x.as_bool()).unwrap_or(c.yolo);
        if let Some(m) = t.get("permission_mode").and_then(|x| x.as_str()) {
            c.permission_mode = m.to_string();
        }
        c.compact_mode = t
            .get("compact_mode")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.compact_mode);
        c.vim_mode = t.get("vim_mode").and_then(|x| x.as_bool()).unwrap_or(c.vim_mode);
        c.show_thinking_blocks = t
            .get("show_thinking_blocks")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.show_thinking_blocks);
        c.simple_mode = t
            .get("simple_mode")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.simple_mode);
        if let Some(m) = t.get("screen_mode").and_then(|x| x.as_str()) {
            c.screen_mode = m.to_string();
        }
        c.page_flip_on_send = t
            .get("page_flip_on_send")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.page_flip_on_send);
        c.remember_tool_approvals = t
            .get("remember_tool_approvals")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.remember_tool_approvals);
        c.group_tool_verbs = t
            .get("group_tool_verbs")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.group_tool_verbs);
        if let Some(n) = t.get("max_thoughts_width").and_then(|x| x.as_integer()) {
            c.max_thoughts_width = n;
        }
    }
    if let Some(t) = v.get("session") {
        if let Some(n) = t
            .get("auto_compact_threshold_percent")
            .and_then(|x| x.as_integer())
        {
            c.auto_compact_threshold_percent = n;
        }
        c.load_envrc = t
            .get("load_envrc")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.load_envrc);
    }
    if let Some(t) = v.get("features") {
        c.telemetry = t
            .get("telemetry")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.telemetry);
        c.feedback = t
            .get("feedback")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.feedback);
        c.lsp_tools = t
            .get("lsp_tools")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.lsp_tools);
        c.codebase_indexing = t
            .get("codebase_indexing")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.codebase_indexing);
        c.remote_fetch = t
            .get("remote_fetch")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.remote_fetch);
    }
    if let Some(t) = v.get("tools") {
        c.respect_gitignore = t
            .get("respect_gitignore")
            .and_then(|x| x.as_bool())
            .unwrap_or(c.respect_gitignore);
    }

    c
}
