//! Windows title bar / chrome theming (dark/light immersive caption).

/// Apply dark or light title bar to match the app theme.
/// Safe to call repeatedly; no-ops on non-Windows.
pub fn apply_titlebar_theme(dark: bool) {
    #[cfg(windows)]
    {
        apply_titlebar_theme_win(dark);
    }
    #[cfg(not(windows))]
    {
        let _ = dark;
    }
}

#[cfg(windows)]
fn apply_titlebar_theme_win(dark: bool) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetForegroundWindow, IsWindow};

    // DWMWA_USE_IMMERSIVE_DARK_MODE = 20 (Win10 1903+)
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

    unsafe {
        let value: i32 = if dark { 1 } else { 0 };
        let mut hwnds: Vec<HWND> = Vec::new();

        // Prefer window titled "Grok" (our viewport title)
        let title: Vec<u16> = OsStr::new("Grok")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let found = FindWindowW(std::ptr::null(), title.as_ptr());
        if IsWindow(found) != 0 {
            hwnds.push(found);
        }

        // Also try foreground (covers first frames / alternate titles)
        let fg = GetForegroundWindow();
        if IsWindow(fg) != 0 && !hwnds.contains(&fg) {
            hwnds.push(fg);
        }

        for hwnd in hwnds {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &value as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}
