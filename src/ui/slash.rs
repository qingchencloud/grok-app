//! Slash command palette — type `/` in the composer to pick actions.

use super::theme;
use egui::{Align, Color32, Frame, Layout, Margin, RichText, Sense, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashAction {
    /// Insert `/name ` into the composer (pass-through to agent).
    InsertPrompt,
    NewChat,
    Settings,
    Logs,
    Status,
    ToggleYolo,
    ClearChat,
    CompactHint,
}

#[derive(Debug, Clone)]
pub struct SlashItem {
    pub name: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
    pub action: SlashAction,
}

/// Built-in host commands + common agent-facing prompts.
/// Static slash registry (English command tokens). Titles/descs are English defaults;
/// UI should prefer localized labels via `crate::i18n` when painting the palette.
pub const SLASH_ITEMS: &[SlashItem] = &[
    SlashItem {
        name: "new",
        title: "New chat",
        desc: "Bind working directory and start a session",
        action: SlashAction::NewChat,
    },
    SlashItem {
        name: "settings",
        title: "Settings",
        desc: "Open settings",
        action: SlashAction::Settings,
    },
    SlashItem {
        name: "status",
        title: "Status",
        desc: "Connection / model / context summary",
        action: SlashAction::Status,
    },
    SlashItem {
        name: "logs",
        title: "Logs",
        desc: "Open runtime logs",
        action: SlashAction::Logs,
    },
    SlashItem {
        name: "yolo",
        title: "Toggle auto-approve",
        desc: "Toggle --always-approve",
        action: SlashAction::ToggleYolo,
    },
    SlashItem {
        name: "clear",
        title: "Clear view",
        desc: "Clear local timeline (does not delete disk session)",
        action: SlashAction::ClearChat,
    },
    SlashItem {
        name: "compact",
        title: "Compact context",
        desc: "Send /compact to the agent if supported",
        action: SlashAction::CompactHint,
    },
    SlashItem {
        name: "plan",
        title: "计划模式",
        desc: "插入 /plan 提示",
        action: SlashAction::InsertPrompt,
    },
    SlashItem {
        name: "goal",
        title: "目标模式",
        desc: "插入 /goal 提示",
        action: SlashAction::InsertPrompt,
    },
    SlashItem {
        name: "help",
        title: "帮助",
        desc: "列出可用斜杠指令",
        action: SlashAction::InsertPrompt,
    },
];

/// Parse filter from composer text when it starts with `/`.
/// Returns (filter fragment without leading slash, true if palette should show).
pub fn slash_filter(input: &str) -> Option<String> {
    let t = input.trim_start();
    if !t.starts_with('/') {
        return None;
    }
    // Only while still on the first token (no space after command yet) or only `/`
    let rest = &t[1..];
    if rest.contains('\n') {
        return None;
    }
    // Hide after user finished a full command + space and continues typing args
    if let Some((cmd, after)) = rest.split_once(' ') {
        if !cmd.is_empty() && !after.is_empty() {
            return None;
        }
        // `/foo ` still show? hide after space
        if rest.ends_with(' ') && !cmd.is_empty() {
            return None;
        }
    }
    let filter = rest.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    Some(filter)
}

pub fn filtered_items(filter: &str) -> Vec<&'static SlashItem> {
    let f = filter.trim().to_ascii_lowercase();
    SLASH_ITEMS
        .iter()
        .filter(|it| {
            if f.is_empty() {
                return true;
            }
            it.name.contains(&f)
                || it.title.to_ascii_lowercase().contains(&f)
                || it.desc.to_ascii_lowercase().contains(&f)
        })
        .collect()
}

/// Draw palette above composer. Returns selected item if clicked / Enter.
pub fn draw_palette(
    ui: &mut Ui,
    filter: &str,
    selected_idx: &mut usize,
) -> Option<&'static SlashItem> {
    let items = filtered_items(filter);
    if items.is_empty() {
        return None;
    }
    if *selected_idx >= items.len() {
        *selected_idx = items.len() - 1;
    }

    let mut picked: Option<&'static SlashItem> = None;
    let w = ui.available_width();

    Frame::NONE
        .fill(if theme::is_dark() {
            theme::SURFACE()
        } else {
            Color32::WHITE
        })
        .stroke(theme::modal_stroke())
        .shadow(theme::elev_shadow())
        .corner_radius(10)
        .inner_margin(Margin::symmetric(6, 6))
        .show(ui, |ui| {
            ui.set_width(w);
            ui.label(
                RichText::new(if filter.is_empty() {
                    "指令".to_string()
                } else {
                    format!("/{filter}")
                })
                .size(11.0)
                .color(theme::TEXT_3()),
            );
            ui.add_space(4.0);

            for (i, item) in items.iter().enumerate() {
                let active = i == *selected_idx;
                let fill = if active {
                    theme::SELECTED()
                } else {
                    Color32::TRANSPARENT
                };
                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(w - 4.0, 40.0),
                    Sense::click().union(Sense::hover()),
                );
                if ui.is_rect_visible(rect) {
                    if fill != Color32::TRANSPARENT || resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            8.0,
                            if resp.hovered() && !active {
                                theme::HOVER()
                            } else {
                                fill
                            },
                        );
                    }
                    ui.painter().text(
                        egui::pos2(rect.left() + 10.0, rect.center().y - 7.0),
                        egui::Align2::LEFT_CENTER,
                        format!("/{}", item.name),
                        egui::FontId::proportional(13.0),
                        if active {
                            theme::ACCENT()
                        } else {
                            theme::TEXT()
                        },
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 10.0, rect.center().y + 9.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{} — {}", item.title, item.desc),
                        egui::FontId::proportional(11.0),
                        theme::TEXT_3(),
                    );
                }
                if resp.hovered() {
                    *selected_idx = i;
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    picked = Some(*item);
                }
            }

            ui.add_space(2.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("↑↓ 选择  ·  Enter 确认  ·  Esc 关闭")
                        .size(10.5)
                        .color(theme::TEXT_3()),
                );
            });
        });

    picked
}

/// Apply keyboard navigation while palette is open.
pub fn handle_keys(
    ctx: &egui::Context,
    filter: &str,
    selected_idx: &mut usize,
) -> Option<&'static SlashItem> {
    let items = filtered_items(filter);
    if items.is_empty() {
        return None;
    }
    if *selected_idx >= items.len() {
        *selected_idx = 0;
    }
    let mut pick = None;
    ctx.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) {
            *selected_idx = (*selected_idx + 1) % items.len();
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            *selected_idx = if *selected_idx == 0 {
                items.len() - 1
            } else {
                *selected_idx - 1
            };
        }
        if i.key_pressed(egui::Key::Enter) && !i.modifiers.shift {
            pick = Some(items[*selected_idx]);
        }
        if i.key_pressed(egui::Key::Escape) {
            // Caller clears input slash mode via None + flag
            pick = None;
        }
    });
    pick
}
