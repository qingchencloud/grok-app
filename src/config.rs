use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_DIR: &str = "GrokApp";
const CONFIG_FILE: &str = "config.json";

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
    pub always_approve: bool,
    /// Extra args prepended before `stdio` (advanced).
    pub extra_agent_args: Vec<String>,
    pub dark_mode: bool,
    /// UI language: `en` | `zh` (English primary, Chinese secondary).
    pub ui_locale: String,
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
            extra_agent_args: Vec::new(),
            dark_mode: true,
            ui_locale: "en".into(),
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
        }
    }
}

impl AppConfig {
    /// Resolved UI locale (`en` / `zh`).
    pub fn locale(&self) -> crate::i18n::Locale {
        crate::i18n::Locale::from_str(&self.ui_locale)
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
        let collapsed: String = raw
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
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
            return crate::i18n::t().me.chars().next().unwrap_or('M').to_string();
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
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
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

    anyhow::bail!(
        crate::i18n::t().grok_not_found
    )
}

pub fn grok_home() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("GROK_HOME") {
        return Some(PathBuf::from(h));
    }
    dirs::home_dir().map(|h| h.join(".grok"))
}

pub fn is_cli_authenticated() -> bool {
    grok_home()
        .map(|h| h.join("auth.json").is_file())
        .unwrap_or(false)
        || std::env::var("XAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false)
}

pub fn path_exists(p: &str) -> bool {
    Path::new(p).exists()
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

