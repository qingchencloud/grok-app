//! Check for newer app builds via GitHub Releases (`qingchencloud/grok-app`).
//!
//! Network runs off the UI thread; results arrive through a channel.

use anyhow::{anyhow, Context, Result};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub const GITHUB_OWNER: &str = "qingchencloud";
pub const GITHUB_REPO: &str = "grok-app";
pub const RELEASES_URL: &str = "https://github.com/qingchencloud/grok-app/releases";
pub const LATEST_URL: &str = "https://github.com/qingchencloud/grok-app/releases/latest";

const API_LATEST: &str =
    "https://api.github.com/repos/qingchencloud/grok-app/releases/latest";
const API_LIST: &str =
    "https://api.github.com/repos/qingchencloud/grok-app/releases?per_page=12";
const USER_AGENT: &str = "GrokDesktop-UpdateCheck/0.1 (+https://github.com/qingchencloud/grok-app)";

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub html_url: String,
    pub published_at: String,
    /// Preferred Windows installer asset URL (Setup.exe).
    pub setup_url: Option<String>,
    /// Portable Windows .exe
    pub portable_url: Option<String>,
    pub setup_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    Ok {
        current: String,
        latest: ReleaseInfo,
        newer: bool,
        history: Vec<ReleaseInfo>,
    },
    Err(String),
}

#[derive(Debug, Default)]
pub struct UpdateUiState {
    pub checking: bool,
    pub current: String,
    pub latest: Option<ReleaseInfo>,
    /// True when GitHub latest > current.
    pub update_available: bool,
    pub history: Vec<ReleaseInfo>,
    pub error: Option<String>,
    pub last_checked: Option<Instant>,
    /// Modal with changelog open.
    pub modal_open: bool,
    /// This process already auto-opened the modal once.
    pub auto_prompted: bool,
    /// User picked a history tag to show in the modal (default: latest).
    pub selected_tag: Option<String>,
}

impl UpdateUiState {
    pub fn new() -> Self {
        Self {
            current: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    pub fn selected_release(&self) -> Option<&ReleaseInfo> {
        let tag = self
            .selected_tag
            .as_deref()
            .or_else(|| self.latest.as_ref().map(|r| r.tag.as_str()))?;
        self.history
            .iter()
            .find(|r| r.tag == tag)
            .or(self.latest.as_ref())
    }

    /// Corner badge should show while an update exists (even after "Later").
    pub fn show_corner_badge(&self) -> bool {
        self.update_available && self.latest.is_some()
    }

    pub fn apply_result(&mut self, res: UpdateCheckResult, dismissed_tag: &str) {
        self.checking = false;
        self.last_checked = Some(Instant::now());
        match res {
            UpdateCheckResult::Ok {
                current,
                latest,
                newer,
                history,
            } => {
                self.current = current;
                self.error = None;
                self.history = history;
                self.update_available = newer;
                self.selected_tag = Some(latest.tag.clone());
                self.latest = Some(latest);
                // Auto-open modal once per process if newer and not dismissed for this tag
                if newer && !self.auto_prompted {
                    let tag = self.latest.as_ref().map(|r| r.tag.as_str()).unwrap_or("");
                    if dismissed_tag.is_empty() || dismissed_tag != tag {
                        self.modal_open = true;
                        self.auto_prompted = true;
                    }
                }
            }
            UpdateCheckResult::Err(e) => {
                self.error = Some(e);
            }
        }
    }
}

/// Channel for background checks.
pub fn channel() -> (Sender<UpdateCheckResult>, Receiver<UpdateCheckResult>) {
    mpsc::channel()
}

/// Spawn a background check (latest + recent history).
///
/// Set env `GROK_DEMO_UPDATE=1` to inject a fake newer release (for UI preview
/// when GitHub has no Releases yet).
pub fn spawn_check(tx: Sender<UpdateCheckResult>, current: String) {
    thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            let res = if std::env::var_os("GROK_DEMO_UPDATE").is_some() {
                demo_update_result(&current)
            } else {
                match fetch_updates(&current) {
                    Ok((latest, history, newer)) => UpdateCheckResult::Ok {
                        current,
                        latest,
                        newer,
                        history,
                    },
                    Err(e) => UpdateCheckResult::Err(format!("{e:#}")),
                }
            };
            let _ = tx.send(res);
        })
        .ok();
}

/// Local-only fake release so the modal / corner badge can be previewed.
fn demo_update_result(current: &str) -> UpdateCheckResult {
    let latest = ReleaseInfo {
        tag: "v0.2.0".into(),
        name: "Grok Desktop 0.2.0".into(),
        body: "\
## What's new / 更新内容

### UI
- System title bar shows app version (`Grok  vX.Y.Z`)
- Bottom-left update badge when a newer build is available
- Changelog modal: download / later (badge stays after Later)

### Updates
- Checks GitHub Releases (`qingchencloud/grok-app`)
- Prefers Windows `Setup.exe` asset when present

### Notes
- This is a **local demo** release (`GROK_DEMO_UPDATE=1`)
- Real checks use the public Releases API
"
        .into(),
        html_url: LATEST_URL.into(),
        published_at: "2026-07-25T00:00:00Z".into(),
        setup_url: Some(format!(
            "https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
        )),
        portable_url: None,
        setup_name: Some("GrokDesktop-Setup-0.2.0.exe".into()),
    };
    let older = ReleaseInfo {
        tag: format!("v{current}"),
        name: format!("Grok Desktop {current}"),
        body: "Previous release (demo history).\n\n- Baseline build for update UI preview".into(),
        html_url: RELEASES_URL.into(),
        published_at: "2026-07-01T00:00:00Z".into(),
        setup_url: None,
        portable_url: None,
        setup_name: None,
    };
    UpdateCheckResult::Ok {
        current: current.to_string(),
        latest: latest.clone(),
        newer: true,
        history: vec![latest, older],
    }
}

fn fetch_updates(current: &str) -> Result<(ReleaseInfo, Vec<ReleaseInfo>, bool)> {
    let latest_raw = http_get_json(API_LATEST)?;
    let latest = parse_release(&latest_raw)?;
    let list_raw = http_get_json(API_LIST).unwrap_or_else(|_| {
        serde_json::json!([])
    });
    let mut history = Vec::new();
    if let Some(arr) = list_raw.as_array() {
        for item in arr {
            if let Ok(r) = parse_release(item) {
                // skip drafts/prereleases already filtered if API default
                if item.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) {
                    continue;
                }
                history.push(r);
            }
        }
    }
    if history.is_empty() {
        history.push(latest.clone());
    }
    let newer = is_newer(&latest.tag, current);
    Ok((latest, history, newer))
}

fn http_get_json(url: &str) -> Result<serde_json::Value> {
    // Prefer ureq if linked; fall back to blocking via std + minimal HTTP is painful.
    // Use `ehttp` is async callback; use std::process curl as last resort.
    // Direct: ureq is already in tree; declare in Cargo.toml.
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .user_agent(USER_AGENT)
        .build();
    let resp = agent
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .with_context(|| format!("GET {url}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(anyhow!("GitHub API HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.into_json().context("parse JSON")?;
    Ok(v)
}

fn parse_release(v: &serde_json::Value) -> Result<ReleaseInfo> {
    let tag = v
        .get("tag_name")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("missing tag_name"))?
        .to_string();
    let name = v
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or(tag.as_str())
        .to_string();
    let body = v
        .get("body")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let html_url = v
        .get("html_url")
        .and_then(|x| x.as_str())
        .unwrap_or(RELEASES_URL)
        .to_string();
    let published_at = v
        .get("published_at")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    let mut setup_url = None;
    let mut setup_name = None;
    let mut portable_url = None;
    if let Some(assets) = v.get("assets").and_then(|a| a.as_array()) {
        for a in assets {
            let n = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let url = a
                .get("browser_download_url")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if url.is_empty() {
                continue;
            }
            let lower = n.to_ascii_lowercase();
            if lower.contains("setup") && lower.ends_with(".exe") {
                setup_url = Some(url.to_string());
                setup_name = Some(n.to_string());
            } else if lower.contains("windows") && lower.ends_with(".exe") && !lower.contains("setup")
            {
                portable_url = Some(url.to_string());
            }
        }
    }

    Ok(ReleaseInfo {
        tag,
        name,
        body,
        html_url,
        published_at,
        setup_url,
        portable_url,
        setup_name,
    })
}

/// Compare semver-ish tags: `v1.2.3` / `1.2.3` / `1.2.3-beta` (pre-release < same numbers without pre).
pub fn is_newer(remote_tag: &str, local: &str) -> bool {
    match (parse_semver(remote_tag), parse_semver(local)) {
        (Some(r), Some(l)) => r > l,
        _ => {
            let r = remote_tag.trim().trim_start_matches('v');
            let l = local.trim().trim_start_matches('v');
            r != l && !r.is_empty()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    /// 0 = release, 1 = has prerelease suffix (sorts lower than plain when equal numbers via inverse)
    pre: u8,
}

fn parse_semver(s: &str) -> Option<SemVer> {
    let s = s.trim().trim_start_matches('v');
    if s.is_empty() {
        return None;
    }
    let (num, pre) = match s.split_once('-') {
        Some((n, _)) => (n, 1u8),
        None => (s, 0u8),
    };
    let mut parts = num.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
    // pre=0 is final; pre=1 is prerelease → final should win for same numbers.
    // Ord: (1,0,0,0) > (1,0,0,1) if we invert pre in comparison.
    // Store pre_inv: final=1, pre=0 so final > pre
    Some(SemVer {
        major,
        minor,
        patch,
        pre: 1 - pre,
    })
}

/// Open a URL in the default browser (best-effort, no console flash).
pub fn open_url(url: &str) {
    crate::spawn_util::open_url(url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_newer() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "v0.2.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }
}
