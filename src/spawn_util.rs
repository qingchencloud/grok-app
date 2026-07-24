//! Process helpers — never flash a console window on Windows GUI builds.

use std::ffi::OsStr;
use std::process::{Command, Stdio};

/// `CREATE_NO_WINDOW` — child gets no console (no black flash).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `Command` that will not open a console window on Windows.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Same as [`command`], with stdin/stdout/stderr defaults for short probes.
pub fn command_piped(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = command(program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Open a URL in the default browser without flashing a `cmd` window.
pub fn open_url(url: &str) {
    if url.trim().is_empty() {
        return;
    }
    #[cfg(windows)]
    {
        // ShellExecuteW — no intermediate console.
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let wide: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let op: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            // HINSTANCE > 32 means success (historical Win16 convention).
            let rc = ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
            if (rc as isize) <= 32 {
                // Fallback: still hide the console.
                let _ = command("cmd")
                    .args(["/C", "start", "", url])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}
