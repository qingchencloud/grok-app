//! One-click install of official Grok CLI:
//!   irm https://x.ai/cli/install.ps1 | iex

use crate::config::{grok_home, resolve_grok_binary};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use tracing::info;

pub const INSTALL_URL: &str = "https://x.ai/cli/install.ps1";

#[derive(Debug, Clone)]
pub enum InstallProgress {
    Started,
    Log(String),
    Finished { ok: bool, message: String },
}

#[derive(Debug, Clone)]
pub struct CliInstallStatus {
    pub installed: bool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub authenticated: bool,
    pub grok_home: Option<PathBuf>,
}

pub fn probe_status(configured_path: &str) -> CliInstallStatus {
    probe_status_ex(configured_path, true)
}

/// Lightweight probe for app startup (skip spawning `grok --version`).
pub fn probe_status_fast(configured_path: &str) -> CliInstallStatus {
    probe_status_ex(configured_path, false)
}

fn probe_status_ex(configured_path: &str, with_version: bool) -> CliInstallStatus {
    let path = resolve_grok_binary(configured_path).ok();
    let version = if with_version {
        path.as_ref().and_then(|p| query_version(p))
    } else {
        None
    };
    let authenticated = crate::config::is_cli_authenticated();
    CliInstallStatus {
        installed: path.is_some(),
        path,
        version,
        authenticated,
        grok_home: grok_home(),
    }
}

fn query_version(bin: &std::path::Path) -> Option<String> {
    // Guard against hangs / weird stdio when launched from Explorer
    let mut child = Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .ok()?;

    // Wait up to 3s
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > std::time::Duration::from_secs(3) => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    let out = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    let line = text
        .lines()
        .next()
        .or_else(|| err.lines().next())
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}

/// Run the official PowerShell installer in a background thread.
/// On non-Windows, falls back to the curl install script when available.
pub fn install_cli(tx: mpsc::Sender<InstallProgress>) {
    std::thread::spawn(move || {
        let _ = tx.send(InstallProgress::Started);
        let result = run_install(&tx);
        match result {
            Ok(msg) => {
                let _ = tx.send(InstallProgress::Finished {
                    ok: true,
                    message: msg,
                });
            }
            Err(e) => {
                let _ = tx.send(InstallProgress::Finished {
                    ok: false,
                    message: format!("{e:#}"),
                });
            }
        }
    });
}

fn run_install(tx: &mpsc::Sender<InstallProgress>) -> Result<String> {
    #[cfg(windows)]
    {
        let _ = tx.send(InstallProgress::Log(format!(
            "执行: irm {INSTALL_URL} | iex"
        )));
        // Use powershell.exe explicitly (not pwsh) for max compatibility with install.ps1
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!("irm {INSTALL_URL} | iex"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("启动 PowerShell 失败")?;

        // Drain stdout/stderr
        if let Some(stdout) = child.stdout.take() {
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(stdout).lines().flatten() {
                    let _ = tx2.send(InstallProgress::Log(line));
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                for line in BufReader::new(stderr).lines().flatten() {
                    let _ = tx2.send(InstallProgress::Log(format!("[err] {line}")));
                }
            });
        }

        let status = child.wait().context("等待安装进程")?;
        if !status.success() {
            bail!("安装脚本退出码: {:?}", status.code());
        }

        // Verify
        let path = resolve_grok_binary("").ok();
        if let Some(p) = path {
            let ver = query_version(&p).unwrap_or_else(|| "unknown".into());
            info!("CLI installed at {} ({ver})", p.display());
            Ok(format!("{}\n{}\n{ver}", crate::i18n::t().install_ok, p.display()))
        } else {
            Ok(
                crate::i18n::t().install_done_refresh
                    .into(),
            )
        }
    }

    #[cfg(not(windows))]
    {
        let _ = tx.send(InstallProgress::Log(
            "curl -fsSL https://x.ai/cli/install.sh | bash".into(),
        ));
        let status = Command::new("bash")
            .args(["-lc", "curl -fsSL https://x.ai/cli/install.sh | bash"])
            .status()
            .context("run install.sh")?;
        if !status.success() {
            bail!("install.sh failed: {:?}", status.code());
        }
        Ok("Install script finished".into())
    }
}
