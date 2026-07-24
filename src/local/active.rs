//! Read `~/.grok/active_sessions.json` — sessions currently held by a live CLI process.
//! Used so the sidebar can show “进行中” without opening that session first.

use crate::config::grok_home;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct ActiveSessionEntry {
    pub session_id: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub opened_at: Option<String>,
}

fn active_path() -> Option<PathBuf> {
    grok_home().map(|h| h.join("active_sessions.json"))
}

/// Session ids with a process still alive (best-effort PID check).
pub fn live_session_ids() -> HashSet<String> {
    let Some(path) = active_path() else {
        return HashSet::new();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<ActiveSessionEntry>>(&raw) else {
        return HashSet::new();
    };
    let mut out = HashSet::new();
    for e in entries {
        let alive = match e.pid {
            Some(pid) if pid > 0 => process_alive(pid),
            // No pid field — still treat as active (file is maintained by CLI)
            None => true,
            _ => false,
        };
        if alive && !e.session_id.is_empty() {
            out.insert(e.session_id);
        }
    }
    out
}

fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        };
        // 0x1000 = PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZE = 0x00100000
        const ACCESS: u32 = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
        unsafe {
            let h = OpenProcess(ACCESS, 0, pid);
            if h.is_null() {
                return false;
            }
            // Wait 0ms: still running if timeout
            let r = WaitForSingleObject(h, 0);
            let _ = CloseHandle(h);
            r == WAIT_TIMEOUT
        }
    }
    #[cfg(not(windows))]
    {
        // /proc/pid on linux; best-effort kill(0)
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}
