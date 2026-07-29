use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_DIR: &str = "GrokApp";
const CONFIG_FILE: &str = "config.json";

/// The three modes in Grok CLI's Shift+Tab cycle.
///
/// ACP uses these exact ids with `session/set_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Normal,
    Plan,
    AlwaysApprove,
}

impl AgentMode {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Plan, Self::AlwaysApprove];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "default",
            Self::Plan => "plan",
            Self::AlwaysApprove => "bypassPermissions",
        }
    }

    pub fn from_id(id: &str) -> Self {
        match id.trim().to_ascii_lowercase().as_str() {
            "plan" => Self::Plan,
            "bypasspermissions" | "always-approve" | "always_approve" | "yolo" => {
                Self::AlwaysApprove
            }
            _ => Self::Normal,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Normal => Self::Plan,
            Self::Plan => Self::AlwaysApprove,
            Self::AlwaysApprove => Self::Normal,
        }
    }

    pub const fn always_approves(self) -> bool {
        matches!(self, Self::AlwaysApprove)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Path to `grok` binary. Empty = auto-detect (~/.grok/bin/grok or PATH).
    pub grok_path: String,
    /// Working directory passed to the agent session.
    pub cwd: String,
    /// Model id (e.g. grok-4.5). Empty = CLI default.
    pub model: String,
    /// Reasoning effort: `low` | `medium` | `high` → CLI `--reasoning-effort`.
    pub effort: String,
    /// Auto-approve all tool executions (`grok agent --always-approve`).
    ///
    /// Kept for backward-compatible config migration. `permission_mode` is the
    /// authoritative value after load.
    pub always_approve: bool,
    /// ACP session mode: `default` | `plan` | `bypassPermissions`.
    pub permission_mode: String,
    /// Extra args prepended before `stdio` (advanced).
    pub extra_agent_args: Vec<String>,
    pub dark_mode: bool,
    /// UI language: `en` | `zh` (English primary, Chinese secondary).
    pub ui_locale: String,
    /// UI locale source: `system` (default) or `manual`.
    ///
    /// This field was added after the initial release. Because `AppConfig`
    /// uses serde defaults, existing installs automatically migrate to system
    /// language until the user explicitly picks English or 中文.
    pub ui_locale_mode: String,
    /// UI font scale (0.85–1.35). Applied on next theme/fonts refresh.
    pub font_scale: f32,
    pub window_width: f32,
    pub window_height: f32,
    /// Connect agent automatically when the app starts.
    pub auto_connect: bool,
    /// Smooth drip-reveal for assistant streaming.
    pub smooth_stream: bool,
    /// Show thought / reasoning blocks in the transcript.
    pub show_thoughts: bool,
    /// Expand tool call rows by default.
    pub expand_tools: bool,
    /// Enter sends (Shift+Enter for newline). When false, Ctrl+Enter sends.
    pub enter_to_send: bool,
    /// Display name shown next to user messages in chat (customizable).
    pub user_display_name: String,
    /// Optional local image path for the user avatar (png/jpg/…). Empty = letter badge.
    pub user_avatar_path: String,
    /// Check GitHub Releases for updates on startup.
    pub check_updates_on_startup: bool,
    /// Tag the user dismissed with "Later" (e.g. `v0.1.1`). Corner badge still shows.
    pub update_dismissed_tag: String,
    /// Show system tray icon.
    pub show_tray: bool,
    /// Close (X) hides to tray instead of quitting.
    pub close_to_tray: bool,
    /// Desktop notification when an agent turn finishes.
    pub notify_on_turn_complete: bool,
    /// Only notify when the window is unfocused or hidden (avoid noise while using the app).
    pub notify_only_when_unfocused: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            grok_path: String::new(),
            cwd: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| ".".into()),
            model: "grok-4.5".into(),
            effort: "medium".into(),
            always_approve: true,
            permission_mode: "bypassPermissions".into(),
            extra_agent_args: Vec::new(),
            dark_mode: true,
            ui_locale: "en".into(),
            ui_locale_mode: "system".into(),
            font_scale: 1.0,
            // Tall enough that the composer is fully visible under topbar + empty state.
            window_width: 1440.0,
            window_height: 920.0,
            auto_connect: true,
            smooth_stream: true,
            show_thoughts: true,
            expand_tools: false,
            enter_to_send: true,
            user_display_name: String::new(),
            user_avatar_path: String::new(),
            check_updates_on_startup: true,
            update_dismissed_tag: String::new(),
            show_tray: true,
            close_to_tray: true,
            notify_on_turn_complete: true,
            notify_only_when_unfocused: true,
        }
    }
}

impl AppConfig {
    fn from_json_with_migration(raw: &str) -> Self {
        let has_permission_mode = serde_json::from_str::<serde_json::Value>(raw)
            .ok()
            .and_then(|value| value.get("permission_mode").cloned())
            .and_then(|value| value.as_str().map(str::to_owned))
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let mut config: Self = serde_json::from_str(raw).unwrap_or_default();
        let mode = if has_permission_mode {
            AgentMode::from_id(&config.permission_mode)
        } else if config.always_approve {
            AgentMode::AlwaysApprove
        } else {
            AgentMode::Normal
        };
        config.set_agent_mode(mode);
        config
    }

    pub fn agent_mode(&self) -> AgentMode {
        if self.permission_mode.trim().is_empty() {
            if self.always_approve {
                AgentMode::AlwaysApprove
            } else {
                AgentMode::Normal
            }
        } else {
            AgentMode::from_id(&self.permission_mode)
        }
    }

    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.permission_mode = mode.id().to_string();
        self.always_approve = mode.always_approves();
    }

    /// Resolved UI locale (`en` / `zh`).
    pub fn locale(&self) -> crate::i18n::Locale {
        if self.ui_locale_mode.eq_ignore_ascii_case("manual") {
            crate::i18n::Locale::from_str(&self.ui_locale)
        } else {
            crate::i18n::system_locale()
        }
    }

    /// Settings picker value: `system` | `en` | `zh`.
    pub fn locale_preference(&self) -> &'static str {
        if self.ui_locale_mode.eq_ignore_ascii_case("manual") {
            self.locale().as_str()
        } else {
            "system"
        }
    }

    /// Display name for chat; empty → localized default ("Me" / "我").
    pub fn resolved_display_name(&self) -> String {
        let s = self.user_display_name.trim();
        if s.is_empty() {
            crate::i18n::t().default_user_name.to_string()
        } else {
            s.to_string()
        }
    }

    /// Sanitize display name for storage: trim, collapse whitespace, hard cap length.
    pub fn sanitize_display_name(raw: &str) -> String {
        let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let s = collapsed.trim();
        if s.is_empty() {
            return String::new();
        }
        // Hard cap by Unicode graphemes-ish (char count is fine for UI overflow).
        const MAX: usize = 24;
        let count = s.chars().count();
        if count <= MAX {
            s.to_string()
        } else {
            let mut out: String = s.chars().take(MAX).collect();
            out.push('…');
            out
        }
    }

    /// First visible character for letter avatar (CJK / Latin safe).
    pub fn avatar_letter(name: &str) -> String {
        let n = name.trim();
        if n.is_empty() {
            return crate::i18n::t()
                .me
                .chars()
                .next()
                .unwrap_or('M')
                .to_string();
        }
        n.chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "M".into())
    }

    /// Extra agent args as a single editable line for settings UI.
    pub fn extra_args_line(&self) -> String {
        self.extra_agent_args.join(" ")
    }

    pub fn set_extra_args_line(&mut self, line: &str) {
        self.extra_agent_args = line
            .split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    pub fn config_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("cannot resolve config directory")?;
        let dir = base.join(APP_DIR);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(CONFIG_FILE))
    }

    pub fn load() -> Self {
        let path = match Self::path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(s) => Self::from_json_with_migration(&s),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Resolve the `grok` executable path.
pub fn resolve_grok_binary(configured: &str) -> Result<PathBuf> {
    if !configured.trim().is_empty() {
        let p = PathBuf::from(configured.trim());
        if p.is_file() {
            return Ok(p);
        }
        anyhow::bail!("configured grok path not found: {}", p.display());
    }

    // ~/.grok/bin/grok(.exe)
    if let Some(home) = dirs::home_dir() {
        let candidates = [
            home.join(".grok").join("bin").join("grok.exe"),
            home.join(".grok").join("bin").join("grok"),
        ];
        for c in candidates {
            if c.is_file() {
                return Ok(c);
            }
        }
    }

    // PATH
    if let Ok(p) = which::which("grok") {
        return Ok(p);
    }

    anyhow::bail!(crate::i18n::t().grok_not_found)
}

pub fn grok_home() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("GROK_HOME") {
        return Some(PathBuf::from(h));
    }
    dirs::home_dir().map(|h| h.join(".grok"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthFileStamp {
    pub len: u64,
    pub modified: Option<std::time::SystemTime>,
}

/// Metadata-only credential snapshot. Never reads or exposes token contents.
pub fn auth_file_stamp() -> Option<AuthFileStamp> {
    let path = grok_home()?.join("auth.json");
    let metadata = std::fs::metadata(path).ok()?;
    Some(AuthFileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

pub fn auth_credentials_changed(
    before: Option<&AuthFileStamp>,
    after: Option<&AuthFileStamp>,
) -> bool {
    before != after
}

pub fn is_authentication_required_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("authentication required")
        || lower.contains("not authenticated")
        || lower.contains("please log in")
        || lower.contains("please login")
}

pub fn is_cli_authenticated() -> bool {
    if std::env::var("XAI_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }

    let Some(path) = grok_home().map(|h| h.join("auth.json")) else {
        return false;
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    auth_json_has_credentials(&value)
}

fn auth_json_has_credentials(value: &serde_json::Value) -> bool {
    fn object_has_token(map: &serde_json::Map<String, serde_json::Value>) -> bool {
        ["key", "access_token", "refresh_token", "api_key"]
            .iter()
            .any(|name| {
                map.get(*name)
                    .and_then(|v| v.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
            })
    }

    match value {
        serde_json::Value::Object(map) => {
            object_has_token(map) || map.values().any(auth_json_has_credentials)
        }
        serde_json::Value::Array(values) => values.iter().any(auth_json_has_credentials),
        _ => false,
    }
}

pub fn path_exists(p: &str) -> bool {
    Path::new(p).exists()
}

#[cfg(test)]
mod tests {
    use super::{auth_json_has_credentials, AgentMode, AppConfig};

    #[test]
    fn auth_json_requires_a_non_empty_credential() {
        assert!(!auth_json_has_credentials(&serde_json::json!({})));
        assert!(!auth_json_has_credentials(&serde_json::json!({
            "https://auth.x.ai::team": {
                "key": " ",
                "refresh_token": ""
            }
        })));
        assert!(auth_json_has_credentials(&serde_json::json!({
            "https://auth.x.ai::team": {
                "key": "TOKEN"
            }
        })));
    }

    #[test]
    fn old_config_defaults_to_system_locale_mode() {
        let config: AppConfig = serde_json::from_value(serde_json::json!({
            "ui_locale": "en"
        }))
        .unwrap();
        assert_eq!(config.ui_locale_mode, "system");
        assert_eq!(config.locale(), crate::i18n::system_locale());
    }

    #[test]
    fn legacy_always_approve_migrates_to_acp_mode() {
        let enabled = AppConfig::from_json_with_migration(r#"{"always_approve":true}"#);
        assert_eq!(enabled.agent_mode(), AgentMode::AlwaysApprove);

        let disabled = AppConfig::from_json_with_migration(r#"{"always_approve":false}"#);
        assert_eq!(disabled.agent_mode(), AgentMode::Normal);
    }

    #[test]
    fn grok_shift_tab_mode_cycle_is_stable() {
        assert_eq!(AgentMode::Normal.next(), AgentMode::Plan);
        assert_eq!(AgentMode::Plan.next(), AgentMode::AlwaysApprove);
        assert_eq!(AgentMode::AlwaysApprove.next(), AgentMode::Normal);
        assert_eq!(AgentMode::AlwaysApprove.id(), "bypassPermissions");
    }
}

/// Well-known model choices for the desktop picker.
pub const MODELS: &[&str] = &[
    "grok-4.5",
    "grok-4",
    "grok-3",
    "grok-3-mini",
    "grok-2-latest",
    "grok-2-vision-latest",
];

/// Reasoning effort ids (`grok agent --reasoning-effort`). Labels via [`effort_label`].
pub const EFFORT_IDS: &[&str] = &["low", "medium", "high"];

/// Legacy alias used by settings pickers: (id, localized full label).
pub fn effort_choices() -> [(&'static str, &'static str); 3] {
    let s = crate::i18n::t();
    [
        ("low", s.effort_low_full),
        ("medium", s.effort_medium_full),
        ("high", s.effort_high_full),
    ]
}

/// @deprecated use [`effort_choices`] — kept for call sites during migration.
pub const EFFORTS: &[(&str, &str)] = &[
    ("low", "Low · faster"),
    ("medium", "Medium · balanced"),
    ("high", "High · deeper"),
];

pub fn normalize_effort(s: &str) -> &'static str {
    match s.trim().to_ascii_lowercase().as_str() {
        "low" | "l" | "minimal" | "min" => "low",
        "high" | "h" | "max" | "maximum" => "high",
        _ => "medium",
    }
}

pub fn effort_label(id: &str) -> &'static str {
    let s = crate::i18n::t();
    match normalize_effort(id) {
        "low" => s.effort_low,
        "high" => s.effort_high,
        _ => s.effort_medium,
    }
}
