//! System tray icon + menu (show / hide / quit).

use anyhow::{Context, Result};
use std::sync::{mpsc, Mutex, Once, OnceLock, RwLock};
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

#[derive(Clone)]
struct MenuIds {
    show: tray_icon::menu::MenuId,
    hide: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// Process-lifetime event bridge.
///
/// tray-icon/muda event handlers are backed by a `OnceCell`: after the first
/// `set_event_handler(Some(...))`, they cannot be replaced or cleared. Keeping
/// the bridge static lets Settings safely disable/recreate the visible tray
/// icon while the callbacks continue routing to the current menu IDs/context.
struct TrayBridge {
    tx: mpsc::Sender<TrayAction>,
    rx: Mutex<mpsc::Receiver<TrayAction>>,
    menu_ids: RwLock<Option<MenuIds>>,
    ctx: RwLock<Option<egui::Context>>,
}

impl TrayBridge {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
            menu_ids: RwLock::new(None),
            ctx: RwLock::new(None),
        }
    }

    fn configure(&self, menu_ids: MenuIds, ctx: egui::Context) {
        *self.menu_ids.write().expect("tray menu ids lock") = Some(menu_ids);
        *self.ctx.write().expect("tray ctx lock") = Some(ctx);
    }

    fn deactivate(&self) {
        *self.menu_ids.write().expect("tray menu ids lock") = None;
        *self.ctx.write().expect("tray ctx lock") = None;
        while self
            .rx
            .lock()
            .expect("tray actions lock")
            .try_recv()
            .is_ok()
        {}
    }

    fn dispatch(&self, action: TrayAction) {
        let _ = self.tx.send(action);
        let ctx = self.ctx.read().expect("tray ctx lock").clone();
        if let Some(ctx) = ctx {
            // Making the viewport visible guarantees winit schedules a frame
            // for Show/Quit even when the hidden window had stopped drawing.
            if matches!(action, TrayAction::Show | TrayAction::Quit) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }
            ctx.request_repaint();
        }
    }

    fn menu_action(&self, event: &MenuEvent) -> Option<TrayAction> {
        let ids = self.menu_ids.read().expect("tray menu ids lock");
        let ids = ids.as_ref()?;
        let id = event.id();
        if id == &ids.show {
            Some(TrayAction::Show)
        } else if id == &ids.hide {
            Some(TrayAction::Hide)
        } else if id == &ids.quit {
            Some(TrayAction::Quit)
        } else {
            None
        }
    }

    fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        let rx = self.rx.lock().expect("tray actions lock");
        while let Ok(action) = rx.try_recv() {
            out.push(action);
        }
        out
    }
}

fn tray_bridge() -> &'static TrayBridge {
    static BRIDGE: OnceLock<TrayBridge> = OnceLock::new();
    BRIDGE.get_or_init(TrayBridge::new)
}

fn install_event_handlers() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let bridge = tray_bridge();
            if let Some(action) = bridge.menu_action(&event) {
                bridge.dispatch(action);
            }
        }));

        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let action = match event {
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => Some(TrayAction::Toggle),
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => Some(TrayAction::Show),
                _ => None,
            };
            if let Some(action) = action {
                tray_bridge().dispatch(action);
            }
        }));
    });
}

pub struct AppTray {
    /// Kept alive for the process lifetime.
    _tray: TrayIcon,
}

impl AppTray {
    /// Create tray icon. Call after the UI is up (Windows event loop running).
    pub fn try_new(tooltip: &str, ctx: egui::Context) -> Result<Self> {
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

        let bridge = tray_bridge();
        bridge.configure(
            MenuIds {
                show: show_id,
                hide: hide_id,
                quit: quit_id,
            },
            ctx,
        );
        install_event_handlers();

        Ok(Self { _tray: tray })
    }

    /// Drain pending tray actions (call once per frame).
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();

        out.extend(tray_bridge().poll());

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

impl Drop for AppTray {
    fn drop(&mut self) {
        tray_bridge().deactivate();
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

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::MenuId;

    fn bridge_with_ids() -> TrayBridge {
        let bridge = TrayBridge::new();
        *bridge.menu_ids.write().expect("tray menu ids lock") = Some(MenuIds {
            show: MenuId::new("show"),
            hide: MenuId::new("hide"),
            quit: MenuId::new("quit"),
        });
        bridge
    }

    #[test]
    fn menu_ids_map_to_expected_actions() {
        let bridge = bridge_with_ids();

        assert_eq!(
            bridge.menu_action(&MenuEvent {
                id: MenuId::new("show"),
            }),
            Some(TrayAction::Show)
        );
        assert_eq!(
            bridge.menu_action(&MenuEvent {
                id: MenuId::new("hide"),
            }),
            Some(TrayAction::Hide)
        );
        assert_eq!(
            bridge.menu_action(&MenuEvent {
                id: MenuId::new("quit"),
            }),
            Some(TrayAction::Quit)
        );
    }

    #[test]
    fn quit_action_is_delivered_through_bridge() {
        let bridge = bridge_with_ids();
        *bridge.ctx.write().expect("tray ctx lock") = Some(egui::Context::default());

        bridge.dispatch(TrayAction::Quit);

        assert_eq!(bridge.poll(), vec![TrayAction::Quit]);
    }
}
