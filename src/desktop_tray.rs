//! System tray icon + menu (show / hide / quit).

use anyhow::{Context, Result};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Actions produced by tray menu / clicks (polled on the UI thread).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Hide,
    Toggle,
    Quit,
}

pub struct AppTray {
    /// Kept alive for the process lifetime.
    _tray: TrayIcon,
    show_id: tray_icon::menu::MenuId,
    hide_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl AppTray {
    /// Create tray icon. Call after the UI is up (Windows event loop running).
    pub fn try_new(tooltip: &str) -> Result<Self> {
        let icon = load_tray_icon().context("tray icon RGBA")?;

        let show = MenuItem::new(crate::i18n::t().tray_show, true, None);
        let hide = MenuItem::new(crate::i18n::t().tray_hide, true, None);
        let quit = MenuItem::new(crate::i18n::t().tray_quit, true, None);
        let show_id = show.id().clone();
        let hide_id = hide.id().clone();
        let quit_id = quit.id().clone();

        let menu = Menu::new();
        menu.append(&show).context("menu show")?;
        menu.append(&hide).context("menu hide")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("menu sep")?;
        menu.append(&quit).context("menu quit")?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()
            .context("TrayIconBuilder")?;

        Ok(Self {
            _tray: tray,
            show_id,
            hide_id,
            quit_id,
        })
    }

    /// Drain pending tray actions (call once per frame).
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id();
            if id == &self.show_id {
                out.push(TrayAction::Show);
            } else if id == &self.hide_id {
                out.push(TrayAction::Hide);
            } else if id == &self.quit_id {
                out.push(TrayAction::Quit);
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => out.push(TrayAction::Toggle),
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => out.push(TrayAction::Show),
                _ => {}
            }
        }

        if out.iter().any(|a| *a == TrayAction::Quit) {
            return vec![TrayAction::Quit];
        }
        if out.is_empty() {
            return out;
        }
        if let Some(last) = out
            .iter()
            .rev()
            .find(|a| matches!(a, TrayAction::Show | TrayAction::Hide | TrayAction::Toggle))
            .copied()
        {
            vec![last]
        } else {
            out
        }
    }
}

fn load_tray_icon() -> Result<Icon> {
    let bytes = include_bytes!("../assets/icon-32.png");
    let img = image::load_from_memory(bytes)
        .or_else(|_| image::load_from_memory(include_bytes!("../assets/icon.png")))
        .context("decode tray png")?
        .into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).context("Icon::from_rgba")
}
