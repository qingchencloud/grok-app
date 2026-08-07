//! Slash command palette — type `/` in the composer to pick actions.

use super::theme;
use crate::acp::AgentCommand;
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
    /// Toggle CLI `auto` permission mode (classifier).
    ToggleAuto,
    ClearChat,
    CompactHint,
    /// Request a live usage/cost refresh from the agent (`/usage`).
    Usage,
}

#[derive(Debug, Clone)]
pub struct SlashItem {
    pub name: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
    pub action: SlashAction,
}

/// Owned palette row (host builtin or agent-advertised).
#[derive(Debug, Clone)]
pub struct SlashEntry {
    pub name: String,
    pub title: String,
    pub desc: String,
    pub action: SlashAction,
    /// True when sourced from agent `available_commands_update`.
    pub from_agent: bool,
}

/// Built-in host commands + common agent-facing prompts.
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
        name: "usage",
        title: "Usage",
        desc: "Ask agent for token usage / cost",
        action: SlashAction::Usage,
    },
    SlashItem {
        name: "logs",
        title: "Logs",
        desc: "Open runtime logs",
        action: SlashAction::Logs,
    },
    SlashItem {
        name: "yolo",
        title: "Toggle always-approve",
        desc: "Toggle always-approve (bypassPermissions)",
        action: SlashAction::ToggleYolo,
    },
    SlashItem {
        name: "auto",
        title: "Toggle auto mode",
        desc: "Classifier auto-allow for safe tools",
        action: SlashAction::ToggleAuto,
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
        title: "Plan mode",
        desc: "Insert /plan hint",
        action: SlashAction::InsertPrompt,
    },
    SlashItem {
        name: "goal",
        title: "Goal",
        desc: "Insert /goal hint",
        action: SlashAction::InsertPrompt,
    },
    SlashItem {
        name: "workflow",
        title: "Workflow",
        desc: "Insert /workflow hint",
        action: SlashAction::InsertPrompt,
    },
    SlashItem {
        name: "help",
        title: "Help",
        desc: "List slash commands",
        action: SlashAction::InsertPrompt,
    },
];

/// Localized title/description for a host slash item.
pub fn slash_labels(item: &SlashItem) -> (&'static str, &'static str) {
    let s = crate::i18n::t();
    match item.name {
        "new" => (s.slash_new_title, s.slash_new_desc),
        "settings" => (s.slash_settings_title, s.slash_settings_desc),
        "status" => (s.slash_status_title, s.slash_status_desc),
        "usage" => (s.slash_usage_title, s.slash_usage_desc),
        "logs" => (s.slash_logs_title, s.slash_logs_desc),
        "yolo" => (s.slash_yolo_title, s.slash_yolo_desc),
        "auto" => (s.slash_auto_title, s.slash_auto_desc),
        "clear" => (s.slash_clear_title, s.slash_clear_desc),
        "compact" => (s.slash_compact_title, s.slash_compact_desc),
        "plan" => (s.slash_plan_title, s.slash_plan_desc),
        "goal" => (s.slash_goal_title, s.slash_goal_desc),
        "workflow" => (s.slash_workflow_title, s.slash_workflow_desc),
        "help" => (s.slash_help_title, s.slash_help_desc),
        _ => (item.title, item.desc),
    }
}

fn host_entry(item: &SlashItem) -> SlashEntry {
    let (title, desc) = slash_labels(item);
    SlashEntry {
        name: item.name.to_string(),
        title: title.to_string(),
        desc: desc.to_string(),
        action: item.action,
        from_agent: false,
    }
}

/// Merge host builtins with agent-advertised commands (host wins on name clash).
pub fn merged_entries(agent_cmds: &[AgentCommand]) -> Vec<SlashEntry> {
    let mut out: Vec<SlashEntry> = SLASH_ITEMS.iter().map(host_entry).collect();
    let host_names: std::collections::HashSet<String> =
        out.iter().map(|e| e.name.to_ascii_lowercase()).collect();
    for c in agent_cmds {
        let name = c.name.trim();
        if name.is_empty() {
            continue;
        }
        if host_names.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        let desc = if c.description.trim().is_empty() {
            c.input_hint.clone().unwrap_or_default()
        } else {
            c.description.clone()
        };
        out.push(SlashEntry {
            name: name.to_string(),
            title: name.to_string(),
            desc,
            action: SlashAction::InsertPrompt,
            from_agent: true,
        });
    }
    out
}

/// Parse filter from composer text when it starts with `/`.
pub fn slash_filter(input: &str) -> Option<String> {
    let t = input.trim_start();
    if !t.starts_with('/') {
        return None;
    }
    let rest = &t[1..];
    if rest.contains('\n') {
        return None;
    }
    if let Some((cmd, after)) = rest.split_once(' ') {
        if !cmd.is_empty() && !after.is_empty() {
            return None;
        }
        if rest.ends_with(' ') && !cmd.is_empty() {
            return None;
        }
    }
    let filter = rest
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(filter)
}

pub fn filtered_entries(filter: &str, agent_cmds: &[AgentCommand]) -> Vec<SlashEntry> {
    let f = filter.trim().to_ascii_lowercase();
    merged_entries(agent_cmds)
        .into_iter()
        .filter(|it| {
            if f.is_empty() {
                return true;
            }
            it.name.to_ascii_lowercase().contains(&f)
                || it.title.to_ascii_lowercase().contains(&f)
                || it.desc.to_ascii_lowercase().contains(&f)
        })
        .collect()
}

/// Draw palette above composer. Returns selected entry if clicked.
pub fn draw_palette(
    ui: &mut Ui,
    filter: &str,
    selected_idx: &mut usize,
    agent_cmds: &[AgentCommand],
) -> Option<SlashEntry> {
    let items = filtered_entries(filter, agent_cmds);
    if items.is_empty() {
        return None;
    }
    if *selected_idx >= items.len() {
        *selected_idx = items.len() - 1;
    }

    let mut picked: Option<SlashEntry> = None;
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
                    crate::i18n::t().slash_commands.to_string()
                } else {
                    format!("/{filter}")
                })
                .size(11.0)
                .color(theme::TEXT_3()),
            );
            ui.add_space(4.0);

            let max_show = 12.min(items.len());
            for (i, item) in items.iter().take(max_show).enumerate() {
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
                    let badge = if item.from_agent { " · cli" } else { "" };
                    ui.painter().text(
                        egui::pos2(rect.left() + 10.0, rect.center().y - 7.0),
                        egui::Align2::LEFT_CENTER,
                        format!("/{}{badge}", item.name),
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
                    picked = Some(item.clone());
                }
            }
            if items.len() > max_show {
                ui.label(
                    RichText::new(format!("… +{} more", items.len() - max_show))
                        .size(11.0)
                        .color(theme::TEXT_3()),
                );
            }

            ui.add_space(2.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hint = match crate::i18n::current_locale() {
                    crate::i18n::Locale::Zh => "↑↓ 选择  ·  Enter 确认  ·  Esc 关闭",
                    crate::i18n::Locale::En => "↑↓ select  ·  Enter confirm  ·  Esc close",
                };
                ui.label(RichText::new(hint).size(10.5).color(theme::TEXT_3()));
            });
        });

    picked
}

/// Apply keyboard navigation while palette is open.
pub fn handle_keys(
    ctx: &egui::Context,
    filter: &str,
    selected_idx: &mut usize,
    agent_cmds: &[AgentCommand],
) -> Option<SlashEntry> {
    let items = filtered_entries(filter, agent_cmds);
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
            pick = Some(items[*selected_idx].clone());
        }
        if i.key_pressed(egui::Key::Escape) {
            pick = None;
        }
    });
    pick
}
