//! Settings — redesigned workbench panel (left nav + clean form sections).

use crate::config::{effort_label, normalize_effort, resolve_grok_binary, AppConfig, MODELS};
use crate::local::install::{probe_status, probe_status_fast, CliInstallStatus, INSTALL_URL};
use crate::local::{CliTomlConfig, LocalSession};
use crate::update::UpdateUiState;
use crate::ui::icons::{self, IconKind};
use crate::ui::theme;
use crate::ui::widgets::{ghost_button, primary_button};
use eframe::egui;
use egui::{
    Align, Color32, Frame, Layout, Margin, Order, RichText, ScrollArea, Sense, Shadow, Stroke,
    TextEdit, Ui, Vec2,
};

const WIN_W: f32 = 880.0;
const WIN_H: f32 = 640.0;
const NAV_W: f32 = 168.0;
/// Settings dialog may use at most this fraction of the viewport.
const VIEW_FRAC: f32 = 0.90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Agent,
    Cli,
    Advanced,
    About,
}

impl SettingsTab {
    pub fn label(self) -> &'static str {
        let s = crate::i18n::t();
        match self {
            Self::Appearance => s.tab_appearance,
            Self::Agent => s.tab_agent,
            Self::Cli => s.tab_cli,
            Self::Advanced => s.tab_advanced,
            Self::About => s.tab_about,
        }
    }

    pub fn hint(self) -> &'static str {
        let s = crate::i18n::t();
        match self {
            Self::Appearance => s.tab_appearance_desc,
            Self::Agent => s.tab_agent_desc,
            Self::Cli => s.tab_cli_desc,
            Self::Advanced => s.tab_advanced_desc,
            Self::About => s.tab_about_desc,
        }
    }

    pub fn all() -> [Self; 5] {
        [
            Self::Appearance,
            Self::Agent,
            Self::Cli,
            Self::Advanced,
            Self::About,
        ]
    }
}

pub struct SettingsState {
    pub open: bool,
    pub tab: SettingsTab,
    pub cli_status: CliInstallStatus,
    pub cli_toml: CliTomlConfig,
    pub install_logs: Vec<String>,
    pub installing: bool,
    pub dirty_toml: bool,
    pub message: Option<String>,
    pub grok_path: String,
    pub cwd: String,
    pub model: String,
    pub effort: String,
    pub always_approve: bool,
    pub dark_mode: bool,
    /// `en` | `zh`
    pub ui_locale: String,
    pub font_scale: f32,
    pub auto_connect: bool,
    pub smooth_stream: bool,
    pub show_thoughts: bool,
    pub expand_tools: bool,
    pub enter_to_send: bool,
    pub extra_args: String,
    /// Editable display name (sanitized on save).
    pub user_display_name: String,
    /// Local image path for chat avatar.
    pub user_avatar_path: String,
    /// Check GitHub Releases for desktop app updates on startup.
    pub check_updates_on_startup: bool,
}

impl SettingsState {
    pub fn new(app: &AppConfig) -> Self {
        let mut s = Self {
            open: false,
            tab: SettingsTab::Appearance,
            cli_status: probe_status_fast(&app.grok_path),
            cli_toml: CliTomlConfig::load(),
            install_logs: Vec::new(),
            installing: false,
            dirty_toml: false,
            message: None,
            grok_path: app.grok_path.clone(),
            cwd: app.cwd.clone(),
            model: app.model.clone(),
            effort: normalize_effort(&app.effort).to_string(),
            always_approve: app.always_approve,
            dark_mode: app.dark_mode,
            ui_locale: app.locale().as_str().to_string(),
            font_scale: app.font_scale,
            auto_connect: app.auto_connect,
            smooth_stream: app.smooth_stream,
            show_thoughts: app.show_thoughts,
            expand_tools: app.expand_tools,
            enter_to_send: app.enter_to_send,
            extra_args: app.extra_args_line(),
            user_display_name: app.user_display_name.clone(),
            user_avatar_path: app.user_avatar_path.clone(),
            check_updates_on_startup: app.check_updates_on_startup,
        };
        if s.model.is_empty() && !s.cli_toml.default_model.is_empty() {
            s.model = s.cli_toml.default_model.clone();
        }
        s
    }

    pub fn sync_from(&mut self, app: &AppConfig) {
        self.grok_path = app.grok_path.clone();
        self.cwd = app.cwd.clone();
        self.model = app.model.clone();
        self.effort = normalize_effort(&app.effort).to_string();
        self.always_approve = app.always_approve;
        self.dark_mode = app.dark_mode;
        self.ui_locale = app.locale().as_str().to_string();
        self.font_scale = app.font_scale;
        self.auto_connect = app.auto_connect;
        self.smooth_stream = app.smooth_stream;
        self.show_thoughts = app.show_thoughts;
        self.expand_tools = app.expand_tools;
        self.enter_to_send = app.enter_to_send;
        self.extra_args = app.extra_args_line();
        self.user_display_name = app.user_display_name.clone();
        self.user_avatar_path = app.user_avatar_path.clone();
        self.check_updates_on_startup = app.check_updates_on_startup;
    }

    pub fn refresh_cli(&mut self) {
        self.cli_status = probe_status(&self.grok_path);
        self.cli_toml = CliTomlConfig::load();
        self.dirty_toml = false;
    }
}

pub struct SettingsActions {
    pub close: bool,
    pub save_agent_and_reconnect: bool,
    pub save_config: bool,
    pub reconnect: bool,
    pub start_install: bool,
    pub open_login: bool,
    pub open_sessions_focus: bool,
    pub apply_theme: bool,
    /// Trigger desktop app update check (GitHub Releases).
    pub check_updates: bool,
    /// Open the update / changelog modal.
    pub open_update_modal: bool,
}

pub fn draw_settings(
    ctx: &egui::Context,
    state: &mut SettingsState,
    sessions: &[LocalSession],
    update: &UpdateUiState,
) -> SettingsActions {
    let mut actions = SettingsActions {
        close: false,
        save_agent_and_reconnect: false,
        save_config: false,
        reconnect: false,
        start_install: false,
        open_login: false,
        open_sessions_focus: false,
        apply_theme: false,
        check_updates: false,
        open_update_modal: false,
    };

    let mut open = state.open;

    // Scrim on Middle layer (above main UI). Window is Foreground so it is NEVER covered.
    if open {
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("settings_scrim"))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .sense(Sense::click())
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, theme::modal_scrim());
                let resp = ui.allocate_rect(screen, Sense::click());
                if resp.clicked() {
                    ui.ctx().memory_mut(|m| {
                        m.data
                            .insert_temp(egui::Id::new("settings_scrim_clicked"), true);
                    });
                }
            });
    }

    let screen = ctx.screen_rect();
    let max_w = (screen.width() * VIEW_FRAC).clamp(640.0, 1100.0);
    let max_h = (screen.height() * VIEW_FRAC).clamp(400.0, 860.0);
    let def_w = WIN_W.min(max_w);
    let def_h = WIN_H.min(max_h);

    let win_resp = egui::Window::new(crate::i18n::t().settings)
        .id(egui::Id::new("win_settings"))
        .open(&mut open)
        // Critical: above the Middle-layer scrim (default Window is also Middle → scrim wins)
        .order(Order::Foreground)
        .default_size([def_w, def_h])
        .min_size([def_w.min(640.0), 360.0])
        .max_size([max_w, max_h])
        .resizable(true)
        .constrain(true)
        .constrain_to(screen)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        // No Frame.shadow — egui's soft shadow often renders as a second solid grey plate.
        // Separation = full-screen scrim only.
        .frame(
            Frame::NONE
                .fill(theme::modal_fill())
                .stroke(theme::modal_stroke())
                .shadow(Shadow::NONE)
                .inner_margin(0.0)
                .corner_radius(14),
        )
        .show(ctx, |ui| {
            // Cap content to window; never expand past 90% viewport
            let full = ui.available_size();
            let main_h = full.y.min(max_h - 8.0).max(280.0);
            let main_w = full.x.min(max_w - 8.0).max(560.0);

            // Toast strip
            if let Some(msg) = state.message.clone() {
                let mut dismiss = false;
                Frame::NONE
                    .fill(if theme::is_dark() {
                        Color32::from_rgba_unmultiplied(40, 120, 70, 55)
                    } else {
                        Color32::from_rgb(220, 252, 231)
                    })
                    .inner_margin(Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(msg).size(13.0).color(theme::SUCCESS()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("×").size(14.0).color(theme::TEXT_3()),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    dismiss = true;
                                }
                            });
                        });
                    });
                if dismiss {
                    state.message = None;
                }
            }

            ui.horizontal_top(|ui| {
                // ── Left nav rail ─────────────────────────────────────
                ui.allocate_ui_with_layout(
                    Vec2::new(NAV_W, main_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(NAV_W);
                        ui.set_min_height(main_h);
                        Frame::NONE
                            .fill(theme::modal_nav_fill())
                            .inner_margin(Margin::symmetric(12, 16))
                            .show(ui, |ui| {
                                ui.set_width(NAV_W - 24.0);
                                ui.set_min_height(main_h - 32.0);
                                ui.horizontal(|ui| {
                                    icons::grok_logo(ui, 18.0);
                                    ui.label(
                                        RichText::new(crate::i18n::t().settings)
                                            .size(14.0)
                                            .strong()
                                            .color(theme::TEXT()),
                                    );
                                });
                                ui.add_space(18.0);

                                for tab in SettingsTab::all() {
                                    let selected = state.tab == tab;
                                    let fill = if selected {
                                        if theme::is_dark() {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 22)
                                        } else {
                                            Color32::WHITE
                                        }
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    let text_c = if selected {
                                        theme::TEXT()
                                    } else {
                                        theme::TEXT_2()
                                    };
                                    // Selected pill: light mode gets soft shadow so it lifts
                                    let resp = if selected && !theme::is_dark() {
                                        Frame::NONE
                                            .fill(fill)
                                            .shadow(egui::Shadow {
                                                offset: [0, 1],
                                                blur: 6,
                                                spread: 0,
                                                color: Color32::from_black_alpha(18),
                                            })
                                            .corner_radius(8)
                                            .show(ui, |ui| {
                                                ui.add_sized(
                                                    [(NAV_W - 28.0).max(100.0), 36.0],
                                                    egui::Button::new(
                                                        RichText::new(tab.label())
                                                            .size(13.5)
                                                            .color(text_c),
                                                    )
                                                    .fill(Color32::TRANSPARENT)
                                                    .stroke(egui::Stroke::NONE),
                                                )
                                            })
                                            .inner
                                    } else {
                                        ui.add_sized(
                                            [(NAV_W - 28.0).max(100.0), 36.0],
                                            egui::Button::new(
                                                RichText::new(tab.label()).size(13.5).color(text_c),
                                            )
                                            .fill(fill)
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(8),
                                        )
                                    };
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        state.tab = tab;
                                    }
                                    ui.add_space(3.0);
                                }

                                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                                    ui.label(
                                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                            .size(11.0)
                                            .color(theme::TEXT_3()),
                                    );
                                });
                            });
                        // Nav | body divider
                        let r = ui.min_rect();
                        ui.painter()
                            .vline(r.right(), r.y_range(), theme::separator_stroke());
                    },
                );

                // ── Right content ─────────────────────────────────────
                let body_w = (main_w - NAV_W - 4.0).max(400.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(body_w, main_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(body_w.min(ui.available_width()));
                        Frame::NONE
                            .fill(theme::modal_fill())
                            .inner_margin(Margin::symmetric(28, 22))
                            .show(ui, |ui| {
                                ui.set_width((body_w - 56.0).max(360.0).min(ui.available_width()));

                                ui.label(
                                    RichText::new(state.tab.label())
                                        .size(20.0)
                                        .strong()
                                        .color(theme::TEXT()),
                                );
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(state.tab.hint())
                                        .size(13.0)
                                        .color(theme::TEXT_3()),
                                );
                                ui.add_space(18.0);

                                let scroll_h = (main_h - 88.0).max(200.0);
                                ScrollArea::vertical()
                                    .id_salt("settings_body_v2")
                                    .max_height(scroll_h)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_max_width((body_w - 64.0).max(340.0));
                                        match state.tab {
                                            SettingsTab::Appearance => {
                                                tab_appearance(ui, state, &mut actions)
                                            }
                                            SettingsTab::Agent => {
                                                tab_agent(ui, state, &mut actions)
                                            }
                                            SettingsTab::Cli => tab_cli(ui, state, &mut actions),
                                            SettingsTab::Advanced => {
                                                tab_advanced(ui, state, sessions, &mut actions)
                                            }
                                            SettingsTab::About => {
                                                tab_about(ui, state, update, &mut actions)
                                            }
                                        }
                                        ui.add_space(24.0);
                                    });
                            });
                    },
                );
            });
        });

    // Close on scrim click only when the click is outside the settings window.
    let scrim_clicked = ctx.memory(|m| {
        m.data
            .get_temp::<bool>(egui::Id::new("settings_scrim_clicked"))
            .unwrap_or(false)
    });
    if scrim_clicked {
        ctx.memory_mut(|m| {
            m.data
                .remove::<bool>(egui::Id::new("settings_scrim_clicked"));
        });
        let on_window = win_resp
            .as_ref()
            .map(|r| {
                let ptr = ctx.input(|i| i.pointer.interact_pos());
                ptr.map(|p| r.response.rect.expand(8.0).contains(p))
                    .unwrap_or(true)
            })
            .unwrap_or(false);
        if !on_window {
            open = false;
        }
    }

    state.open = open;
    if !open {
        actions.close = true;
    }
    actions
}

// ── shared form bits ─────────────────────────────────────────────────────────

fn section(ui: &mut Ui, title: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    ui.label(
        RichText::new(title)
            .size(13.0)
            .strong()
            .color(theme::TEXT()),
    );
    if !hint.is_empty() {
        ui.add_space(2.0);
        ui.label(RichText::new(hint).size(12.0).color(theme::TEXT_3()));
    }
    ui.add_space(10.0);
    // Elevated card: light mode white + shadow + hairline; dark mode raised surface
    let fill = if theme::is_dark() {
        theme::SURFACE()
    } else {
        Color32::from_rgb(0xFA, 0xFA, 0xFB)
    };
    Frame::NONE
        .fill(fill)
        .stroke(if theme::is_dark() {
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 12))
        } else {
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 14))
        })
        .shadow(theme::card_shadow())
        .corner_radius(12)
        .inner_margin(Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(16.0);
}

fn row_toggle(ui: &mut Ui, label: &str, hint: &str, value: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(13.5).color(theme::TEXT()));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(11.5).color(theme::TEXT_3()));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.checkbox(value, "").changed() {
                changed = true;
            }
        });
    });
    changed
}

fn theme_choice(ui: &mut Ui, active: bool, icon: IconKind, label: &str) -> egui::Response {
    let fill = if active {
        theme::accent_soft()
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        Stroke::new(1.0, theme::ACCENT())
    } else {
        Stroke::new(1.0, theme::BORDER())
    };
    let color = if active {
        theme::ACCENT()
    } else {
        theme::TEXT_2()
    };
    ui.add(
        egui::Button::new(
            RichText::new(format!(
                "{} {}",
                match icon {
                    IconKind::Sun => "☀",
                    IconKind::Moon => "☾",
                    _ => "·",
                },
                label
            ))
            .size(12.5)
            .color(color),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(8)
        .min_size(Vec2::new(88.0, 32.0)),
    )
}

fn field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(theme::TEXT_2()));
    ui.add_space(4.0);
}

fn mono_path(ui: &mut Ui, path: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(path)
                .size(11.5)
                .monospace()
                .color(theme::TEXT_3()),
        )
        .truncate()
        .sense(Sense::hover()),
    )
    .on_hover_text(path);
}

// ── tabs ─────────────────────────────────────────────────────────────────────

fn tab_appearance(ui: &mut Ui, state: &mut SettingsState, actions: &mut SettingsActions) {
    let s = crate::i18n::t();
    section(ui, s.language, s.language_hint, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (id, label) in [("en", "English"), ("zh", "中文")] {
                let on = state.ui_locale == id;
                let fill = if on {
                    theme::accent_soft()
                } else {
                    Color32::TRANSPARENT
                };
                let stroke = if on {
                    Stroke::new(1.0, theme::ACCENT())
                } else {
                    Stroke::new(1.0, theme::BORDER())
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).size(12.5).color(if on {
                            theme::ACCENT()
                        } else {
                            theme::TEXT_2()
                        }))
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(8)
                        .min_size(Vec2::new(88.0, 32.0)),
                    )
                    .clicked()
                {
                    state.ui_locale = id.into();
                    crate::i18n::set_locale(crate::i18n::Locale::from_str(id));
                    actions.apply_theme = true; // repaint chrome
                }
            }
        });
    });

    section(ui, s.theme, s.theme_hint, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let before = state.dark_mode;
            let day = theme_choice(ui, !state.dark_mode, IconKind::Sun, s.day);
            let night = theme_choice(ui, state.dark_mode, IconKind::Moon, s.night);
            if day.clicked() {
                state.dark_mode = false;
            }
            if night.clicked() {
                state.dark_mode = true;
            }
            if state.dark_mode != before {
                actions.apply_theme = true;
            }
        });
    });

    section(ui, s.profile, s.profile_hint, |ui| {
        let s = crate::i18n::t();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let letter = crate::config::AppConfig::avatar_letter(&state.user_display_name);
            let path = if state.user_avatar_path.trim().is_empty() {
                None
            } else {
                Some(state.user_avatar_path.as_str())
            };
            icons::user_avatar_ex(ui, 40.0, &letter, path);
            ui.vertical(|ui| {
                field_label(ui, s.display_name);
                let resp = ui.add(
                    TextEdit::singleline(&mut state.user_display_name)
                        .desired_width(220.0)
                        .char_limit(32)
                        .hint_text(s.display_name_hint),
                );
                if resp.changed() && state.user_display_name.chars().count() > 32 {
                    state.user_display_name = state.user_display_name.chars().take(32).collect();
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "{} / 24 {}",
                        state.user_display_name.chars().count().min(24),
                        s.name_chars_hint
                    ))
                    .size(11.0)
                    .color(theme::TEXT_3()),
                );
            });
        });
        ui.add_space(12.0);
        field_label(ui, s.avatar_image);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ghost_button(ui, s.choose_image).clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter(
                        s.images_filter,
                        &["png", "jpg", "jpeg", "gif", "webp", "bmp"],
                    )
                    .pick_file()
                {
                    state.user_avatar_path = p.display().to_string();
                }
            }
            if !state.user_avatar_path.is_empty() && ghost_button(ui, s.clear).clicked() {
                state.user_avatar_path.clear();
            }
        });
        if !state.user_avatar_path.is_empty() {
            ui.add_space(4.0);
            mono_path(ui, &state.user_avatar_path);
        } else {
            ui.add_space(2.0);
            ui.label(
                RichText::new(s.avatar_unset_hint)
                    .size(11.5)
                    .color(theme::TEXT_3()),
            );
        }
    });

    section(ui, s.font_size, "", |ui| {
        let s = crate::i18n::t();
        ui.horizontal(|ui| {
            field_label(ui, s.font_scale);
            ui.add(
                egui::Slider::new(&mut state.font_scale, 0.85..=1.35)
                    .fixed_decimals(2)
                    .suffix("x"),
            );
            if ghost_button(ui, s.reset).clicked() {
                state.font_scale = 1.0;
            }
        });
    });

    section(ui, s.chat_experience, "", |ui| {
        let s = crate::i18n::t();
        let _ = row_toggle(
            ui,
            s.smooth_stream,
            s.smooth_stream_hint,
            &mut state.smooth_stream,
        );
        ui.add_space(10.0);
        let _ = row_toggle(
            ui,
            s.show_thoughts,
            s.show_thoughts_hint,
            &mut state.show_thoughts,
        );
        ui.add_space(10.0);
        let _ = row_toggle(
            ui,
            s.expand_tools,
            s.expand_tools_hint,
            &mut state.expand_tools,
        );
        ui.add_space(10.0);
        let _ = row_toggle(
            ui,
            s.enter_to_send,
            s.enter_to_send_hint,
            &mut state.enter_to_send,
        );
    });
}

fn tab_agent(ui: &mut Ui, state: &mut SettingsState, actions: &mut SettingsActions) {
    section(
        ui,
        crate::i18n::t().model,
        crate::i18n::t().model_hint,
        |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("set_model")
                    .selected_text(if state.model.is_empty() {
                        crate::i18n::t().select_model
                    } else {
                        &state.model
                    })
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for m in MODELS {
                            ui.selectable_value(&mut state.model, (*m).to_string(), *m);
                        }
                    });
                ui.add(
                    TextEdit::singleline(&mut state.model)
                        .desired_width(ui.available_width().max(120.0))
                        .hint_text(crate::i18n::t().or_type_model),
                );
            });
        },
    );

    section(
        ui,
        crate::i18n::t().effort_heading,
        crate::i18n::t().effort_hint,
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (id, label) in crate::config::effort_choices() {
                    let active = normalize_effort(&state.effort) == id;
                    let fill = if active {
                        theme::SELECTED()
                    } else {
                        Color32::TRANSPARENT
                    };
                    let resp = ui.add(
                        egui::Button::new(RichText::new(label).size(12.5).color(if active {
                            theme::TEXT()
                        } else {
                            theme::TEXT_2()
                        }))
                        .fill(fill)
                        .stroke(if active {
                            egui::Stroke::NONE
                        } else {
                            egui::Stroke::new(1.0, theme::DIVIDER())
                        })
                        .corner_radius(8)
                        .min_size(egui::vec2(88.0, 32.0)),
                    );
                    if resp.clicked() {
                        state.effort = id.to_string();
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(crate::i18n::current_effort_line(
                    effort_label(&state.effort),
                    normalize_effort(&state.effort),
                ))
                .size(12.0)
                .color(theme::TEXT_3()),
            );
        },
    );

    section(
        ui,
        crate::i18n::t().working_dir,
        crate::i18n::t().working_dir_hint,
        |ui| {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 48.0).max(160.0);
                ui.add(
                    TextEdit::singleline(&mut state.cwd)
                        .desired_width(w)
                        .hint_text(crate::i18n::t().working_dir),
                );
                if ui
                    .add_sized([40.0, 28.0], egui::Button::new("…"))
                    .on_hover_text(crate::i18n::t().pick_folder)
                    .clicked()
                {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.cwd = dir.display().to_string();
                    }
                }
            });
        },
    );

    section(ui, crate::i18n::t().permissions, "", |ui| {
        row_toggle(
            ui,
            crate::i18n::t().always_approve,
            crate::i18n::t().always_approve_hint,
            &mut state.always_approve,
        );
        ui.add_space(10.0);
        row_toggle(
            ui,
            crate::i18n::t().auto_connect,
            crate::i18n::t().auto_connect_hint,
            &mut state.auto_connect,
        );
    });

    section(
        ui,
        crate::i18n::t().grok_binary,
        crate::i18n::t().grok_binary_hint,
        |ui| {
            ui.add(
                TextEdit::singleline(&mut state.grok_path)
                    .desired_width(ui.available_width())
                    .hint_text(crate::i18n::t().auto_detect),
            );
            if let Ok(p) = resolve_grok_binary(if state.grok_path.is_empty() {
                ""
            } else {
                &state.grok_path
            }) {
                ui.add_space(6.0);
                mono_path(ui, &format!("✓ {}", p.display()));
            }
        },
    );

    ui.horizontal(|ui| {
        if primary_button(ui, crate::i18n::t().save_and_reconnect, true).clicked() {
            actions.save_agent_and_reconnect = true;
        }
        if ghost_button(ui, crate::i18n::t().save_only).clicked() {
            actions.save_config = true;
            actions.reconnect = false;
        }
    });
}

fn tab_cli(ui: &mut Ui, state: &mut SettingsState, actions: &mut SettingsActions) {
    let installed = state.cli_status.installed;
    let version = state.cli_status.version.clone();
    let path_disp = state
        .cli_status
        .path
        .as_ref()
        .map(|p| p.display().to_string());
    let authenticated = state.cli_status.authenticated;
    let home_disp = state
        .cli_status
        .grok_home
        .as_ref()
        .map(|p| p.display().to_string());

    section(
        ui,
        crate::i18n::t().cli_status,
        crate::i18n::t().cli_status_hint,
        |ui| {
            ui.horizontal(|ui| {
                let (c, t) = if installed {
                    (theme::SUCCESS(), crate::i18n::t().cli_installed)
                } else {
                    (theme::WARNING(), crate::i18n::t().cli_missing)
                };
                ui.colored_label(c, format!("● {t}"));
                if let Some(v) = &version {
                    ui.label(RichText::new(v).size(12.5).color(theme::TEXT_3()));
                }
            });
            if let Some(p) = &path_disp {
                ui.add_space(4.0);
                mono_path(ui, p);
            }
            ui.add_space(8.0);
            ui.colored_label(
                if authenticated {
                    theme::SUCCESS()
                } else {
                    theme::WARNING()
                },
                if authenticated {
                    format!("● {}", crate::i18n::t().logged_in)
                } else {
                    format!("○ {}", crate::i18n::t().not_logged_in)
                },
            );
            if let Some(h) = &home_disp {
                ui.add_space(4.0);
                mono_path(ui, &format!("GROK_HOME: {h}"));
            }
        },
    );

    section(
        ui,
        crate::i18n::t().install_update,
        &format!("PowerShell: irm {INSTALL_URL} | iex"),
        |ui| {
            ui.horizontal_wrapped(|ui| {
                let label = if state.installing {
                    crate::i18n::t().installing
                } else if installed {
                    crate::i18n::t().reinstall_update
                } else {
                    crate::i18n::t().install_cli_once
                };
                if primary_button(ui, label, !state.installing).clicked() {
                    actions.start_install = true;
                }
                if ghost_button(ui, crate::i18n::t().refresh).clicked() {
                    state.refresh_cli();
                    state.message = Some(crate::i18n::t().status_refreshed.into());
                }
                if ghost_button(ui, crate::i18n::t().login).clicked() {
                    actions.open_login = true;
                }
            });
        },
    );

    if !state.install_logs.is_empty() {
        section(ui, crate::i18n::t().install_log, "", |ui| {
            ScrollArea::vertical()
                .id_salt("install_logs_v2")
                .max_height(160.0)
                .show(ui, |ui| {
                    for (i, line) in state.install_logs.iter().enumerate() {
                        ui.push_id(("ilog", i), |ui| {
                            ui.label(
                                RichText::new(line)
                                    .size(11.0)
                                    .monospace()
                                    .color(theme::TEXT_3()),
                            );
                        });
                    }
                });
        });
    }
}

fn tab_advanced(
    ui: &mut Ui,
    state: &mut SettingsState,
    sessions: &[LocalSession],
    actions: &mut SettingsActions,
) {
    section(
        ui,
        crate::i18n::t().extra_agent_args,
        crate::i18n::t().extra_agent_args_hint,
        |ui| {
            ui.add(
                TextEdit::singleline(&mut state.extra_args)
                    .desired_width(ui.available_width())
                    .hint_text(crate::i18n::t().extra_args_example),
            );
        },
    );

    section(
        ui,
        crate::i18n::t().cli_config_toml,
        crate::i18n::t().shared_with_tui,
        |ui| {
            if let Some(p) = state.cli_toml.path.clone() {
                mono_path(ui, &p.display().to_string());
                ui.add_space(8.0);
            }
            let c = &mut state.cli_toml;
            let s = crate::i18n::t();
            if ui
                .add(
                    egui::Slider::new(&mut c.auto_compact_threshold_percent, 50..=95)
                        .text(s.auto_compact_threshold),
                )
                .changed()
            {
                state.dirty_toml = true;
            }
            ui.add_space(6.0);
            if ui.checkbox(&mut c.yolo, s.yolo_global).changed() {
                state.dirty_toml = true;
            }
            if ui
                .checkbox(&mut c.remember_tool_approvals, s.remember_tool_approvals)
                .changed()
            {
                state.dirty_toml = true;
            }
            if ui.checkbox(&mut c.load_envrc, s.load_envrc).changed() {
                state.dirty_toml = true;
            }
            if ui
                .checkbox(&mut c.show_thinking_blocks, s.show_thinking_tui)
                .changed()
            {
                state.dirty_toml = true;
            }
            if ui
                .checkbox(&mut c.codebase_indexing, s.codebase_indexing)
                .changed()
            {
                state.dirty_toml = true;
            }
            if ui.checkbox(&mut c.auto_update, s.cli_auto_update).changed() {
                state.dirty_toml = true;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                field_label(ui, s.default_model);
                if ui
                    .add(
                        TextEdit::singleline(&mut c.default_model)
                            .desired_width(180.0)
                            .hint_text("grok-4.5"),
                    )
                    .changed()
                {
                    state.dirty_toml = true;
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                field_label(ui, s.permission_mode);
                egui::ComboBox::from_id_salt("perm_mode_v2")
                    .selected_text(&c.permission_mode)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for m in [
                            "default",
                            "always-approve",
                            "acceptEdits",
                            "auto",
                            "dontAsk",
                            "bypassPermissions",
                            "plan",
                        ] {
                            if ui
                                .selectable_value(&mut c.permission_mode, m.to_string(), m)
                                .changed()
                            {
                                state.dirty_toml = true;
                            }
                        }
                    });
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if primary_button(ui, crate::i18n::t().save_config_toml, true).clicked() {
                    match state.cli_toml.save() {
                        Ok(()) => {
                            state.dirty_toml = false;
                            state.cli_toml.loaded = true;
                            state.message = Some(crate::i18n::t().wrote_config_toml.into());
                        }
                        Err(e) => state.message = Some(crate::i18n::save_failed(&e)),
                    }
                }
                if ghost_button(ui, crate::i18n::t().reload).clicked() {
                    state.cli_toml = CliTomlConfig::load();
                    state.dirty_toml = false;
                    state.message = Some(crate::i18n::t().reloaded_from_disk.into());
                }
                if state.dirty_toml {
                    ui.label(
                        RichText::new(crate::i18n::t().unsaved)
                            .size(11.5)
                            .color(theme::WARNING()),
                    );
                }
            });
        },
    );

    section(
        ui,
        crate::i18n::t().local_sessions,
        &crate::i18n::sessions_count_line(sessions.len()),
        |ui| {
            if ghost_button(ui, crate::i18n::t().view_in_sidebar).clicked() {
                actions.open_sessions_focus = true;
            }
            ui.add_space(8.0);
            for (i, s) in sessions.iter().take(8).enumerate() {
                ui.push_id(("set_sess", i, s.id.as_str()), |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&s.title).size(12.5).color(theme::TEXT()),
                            )
                            .truncate(),
                        );
                        ui.label(
                            RichText::new(
                                s.updated_at
                                    .map(|t| t.format("%m-%d %H:%M").to_string())
                                    .unwrap_or_else(|| "—".into()),
                            )
                            .size(11.0)
                            .color(theme::TEXT_3()),
                        );
                    });
                });
            }
        },
    );

    footer_save(ui, actions, false);
}

fn tab_about(
    ui: &mut Ui,
    state: &mut SettingsState,
    update: &UpdateUiState,
    actions: &mut SettingsActions,
) {
    let s = crate::i18n::t();
    section(ui, s.app_name, "", |ui| {
        ui.horizontal(|ui| {
            icons::grok_logo(ui, 36.0);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(crate::i18n::version_line(env!("CARGO_PKG_VERSION")))
                        .size(16.0)
                        .strong()
                        .color(theme::TEXT()),
                );
                ui.label(
                    RichText::new("Desktop UI → ACP → grok agent stdio")
                        .size(12.5)
                        .color(theme::TEXT_2()),
                );
            });
        });
        ui.add_space(10.0);
        ui.label(
            RichText::new(s.auth_reuse_cli)
                .size(12.5)
                .color(theme::TEXT_3()),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(s.unofficial_notice)
                .size(12.0)
                .color(theme::TEXT_3()),
        );
    });

    section(ui, s.update_section, s.update_check, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{}  v{}", s.update_current, update.current))
                    .size(13.0)
                    .color(theme::TEXT_2()),
            );
            if let Some(latest) = &update.latest {
                ui.label(RichText::new("→").size(13.0).color(theme::TEXT_3()));
                ui.label(
                    RichText::new(format!("{}  {}", s.update_latest, latest.tag))
                        .size(13.0)
                        .color(if update.update_available {
                            theme::ACCENT()
                        } else {
                            theme::TEXT_2()
                        }),
                );
            }
        });
        ui.add_space(8.0);
        if update.checking {
            ui.label(
                RichText::new(s.update_checking)
                    .size(13.0)
                    .color(theme::TEXT_3()),
            );
        } else if let Some(err) = &update.error {
            ui.label(
                RichText::new(format!("{}: {err}", s.update_failed))
                    .size(12.5)
                    .color(theme::DANGER()),
            );
        } else if update.update_available {
            ui.label(
                RichText::new(s.update_available)
                    .size(13.0)
                    .strong()
                    .color(theme::ACCENT()),
            );
        } else if update.last_checked.is_some() {
            ui.label(
                RichText::new(s.update_up_to_date)
                    .size(13.0)
                    .color(theme::SUCCESS()),
            );
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if primary_button(ui, s.update_check, !update.checking).clicked() {
                actions.check_updates = true;
            }
            if update.update_available && ghost_button(ui, s.update_view).clicked() {
                actions.open_update_modal = true;
            }
            if ghost_button(ui, s.update_open_releases).clicked() {
                crate::update::open_url(crate::update::RELEASES_URL);
            }
        });
        ui.add_space(10.0);
        ui.checkbox(
            &mut state.check_updates_on_startup,
            RichText::new(s.update_check_on_startup).size(13.0),
        );
    });

    section(ui, s.links, "", |ui| {
        ui.hyperlink_to(s.link_xai_cli, "https://x.ai/cli");
        ui.hyperlink_to(s.link_grok_build, "https://github.com/xai-org/grok-build");
        ui.hyperlink_to(
            s.ref_client,
            "https://github.com/qingchencloud/grok-app",
        );
        ui.hyperlink_to(
            s.update_open_releases,
            crate::update::RELEASES_URL,
        );
    });

    section(ui, s.capabilities, "", |ui| {
        ui.label(
            RichText::new(s.about_capabilities_body)
                .size(13.0)
                .color(theme::TEXT_2()),
        );
    });
}

fn footer_save(ui: &mut Ui, actions: &mut SettingsActions, reconnect: bool) {
    ui.horizontal(|ui| {
        if primary_button(ui, crate::i18n::t().save_settings, true).clicked() {
            actions.save_config = true;
            actions.reconnect = reconnect;
        }
    });
}
