//! Chat transcript — clean coding-agent layout (Cursor-like).
//!
//! Layout principles:
//! - **User**: right-aligned bubble glued to avatar (Align::Max — never float left)
//! - **Assistant**: compact Grok header + full-width prose (no heavy card for short text)
//! - **Tools / thoughts**: muted one-line activity; no noisy chrome
//! - Streaming markdown; tables use custom grid

use super::icons;
use super::theme;
use crate::acp::{ChatImage, TimelineItem};
use crate::attachments;
use crate::config::AppConfig;
use egui::{Align, Color32, Frame, Layout, Margin, RichText, Sense, Stroke, TextureHandle, Ui, Vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// Profile shown beside chat bubbles (from settings).
#[derive(Clone, Copy)]
pub struct ChatIdentity<'a> {
    pub user_name: &'a str,
    pub user_avatar_path: &'a str,
}

impl ChatIdentity<'_> {
    pub fn from_config(cfg: &AppConfig) -> ChatIdentity<'_> {
        // Prefer configured name; empty → localized default via static str pool.
        let name = if cfg.user_display_name.trim().is_empty() {
            crate::i18n::t().default_user_name
        } else {
            cfg.user_display_name.as_str()
        };
        ChatIdentity {
            user_name: name,
            user_avatar_path: cfg.user_avatar_path.as_str(),
        }
    }
}

/// Max timeline rows rendered by default (older ones collapsed).
pub const DEFAULT_VISIBLE_ITEMS: usize = 80;

pub use crate::session_store::TurnPhase;

/// Compact status rail at top of the message column — always readable at a glance.
pub fn render_turn_status(
    ui: &mut Ui,
    col_w: f32,
    phase: TurnPhase,
    live_tool: Option<&(String, String)>,
    // Elapsed seconds of current busy turn (None when idle).
    elapsed_secs: Option<u64>,
) {
    if phase == TurnPhase::Idle {
        return;
    }
    let p = theme::t();
    let (dot, fill, stroke_c) = match phase {
        TurnPhase::Permission => (
            p.warning,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(240, 150, 40, 28)
            } else {
                Color32::from_rgb(255, 246, 230)
            },
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(240, 150, 40, 70)
            } else {
                Color32::from_rgb(245, 200, 140)
            },
        ),
        TurnPhase::Tool => (
            p.accent,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(100, 140, 255, 22)
            } else {
                Color32::from_rgb(240, 244, 255)
            },
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(100, 140, 255, 55)
            } else {
                Color32::from_rgb(200, 214, 245)
            },
        ),
        TurnPhase::Thinking => (
            p.text_2,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 10)
            } else {
                Color32::from_rgb(0xF5, 0xF5, 0xF7)
            },
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 18)
            } else {
                Color32::from_rgb(0xE8, 0xE8, 0xEC)
            },
        ),
        TurnPhase::Generating => (
            p.accent,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(100, 140, 255, 18)
            } else {
                Color32::from_rgb(0xF0, 0xF4, 0xFF)
            },
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(100, 140, 255, 45)
            } else {
                Color32::from_rgb(0xD0, 0xDC, 0xF5)
            },
        ),
        TurnPhase::Idle => return,
    };

    let s = crate::i18n::t();
    let detail = match phase {
        TurnPhase::Tool => live_tool
            .map(|(t, _)| t.as_str())
            .unwrap_or(s.tool_default),
        TurnPhase::Permission => s.phase_permission,
        TurnPhase::Thinking => s.phase_thinking,
        TurnPhase::Generating => {
            if live_tool.is_some() {
                s.phase_processing
            } else {
                s.phase_waiting
            }
        }
        TurnPhase::Idle => "",
    };

    let time_s = elapsed_secs.map(|s| {
        if s >= 60 {
            format!("{}:{:02}", s / 60, s % 60)
        } else {
            format!("{s}s")
        }
    });

    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke_c))
        .corner_radius(10)
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(col_w.min(ui.available_width()));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(egui::Spinner::new().size(12.0).color(dot));
                // Phase chip
                Frame::NONE
                    .fill(if theme::is_dark() {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 40)
                    } else {
                        Color32::from_white_alpha(180)
                    })
                    .corner_radius(6)
                    .inner_margin(Margin::symmetric(7, 2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(phase.label())
                                .size(11.5)
                                .strong()
                                .color(dot),
                        );
                    });
                ui.add(
                    egui::Label::new(RichText::new(detail).size(12.0).color(theme::TEXT_2()))
                        .truncate(),
                );
                if let Some(t) = time_s {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(t)
                                .size(11.5)
                                .monospace()
                                .color(theme::TEXT_3()),
                        );
                    });
                }
            });
        });
    ui.add_space(10.0);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}

pub fn render_timeline(
    ui: &mut Ui,
    items: &[TimelineItem],
    md_cache: &mut CommonMarkCache,
    empty: bool,
    mut on_suggestion: impl FnMut(String),
    mut on_image: impl FnMut(ChatImage),
    mut resolve_tex: impl FnMut(&ChatImage) -> Option<TextureHandle>,
    display_assistant: Option<&str>,
    open_assistant_id: Option<&str>,
    _live_tool: Option<(&str, &str)>,
    show_thoughts: bool,
    expand_tools: bool,
    // When false, only the last DEFAULT_VISIBLE_ITEMS are shown.
    show_all_history: &mut bool,
    identity: ChatIdentity<'_>,
    // One-shot: scroll so this timeline item id is at the top of the viewport.
    scroll_to_item_id: Option<&str>,
) {
    let w = ui.available_width().max(200.0);
    ui.set_width(w);
    ui.set_max_width(w);
    ui.set_min_width(w.min(ui.available_width()));

    if empty {
        render_empty(ui, w, &mut on_suggestion);
        return;
    }

    // Filter thoughts if hidden, then window
    let indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            show_thoughts || !matches!(item, TimelineItem::Thought { .. })
        })
        .map(|(i, _)| i)
        .collect();

    let total = indices.len();
    let (skip, visible) = if *show_all_history || total <= DEFAULT_VISIBLE_ITEMS {
        (0usize, indices.as_slice())
    } else {
        let skip = total - DEFAULT_VISIBLE_ITEMS;
        (skip, &indices[skip..])
    };

    if skip > 0 {
        ui.add_space(4.0);
        let p = theme::t();
        Frame::NONE
            .fill(if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 8)
            } else {
                Color32::from_rgb(0xF0, 0xF1, 0xF4)
            })
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(10)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(w.min(ui.available_width()));
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "↑ {skip} {}",
                            crate::i18n::t().history_collapsed
                        ))
                        .size(12.0)
                        .color(p.text_2),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(crate::i18n::t().show_all)
                                        .size(12.0)
                                        .strong()
                                        .color(p.accent),
                                )
                                .fill(theme::accent_soft())
                                .stroke(Stroke::NONE)
                                .corner_radius(6)
                                .min_size(Vec2::new(64.0, 24.0)),
                            )
                            .clicked()
                        {
                            *show_all_history = true;
                        }
                    });
                });
            });
        ui.add_space(12.0);
    }

    for (vi, &idx) in visible.iter().enumerate() {
        let item = &items[idx];
        // Chain continuity: tool/status rows share a left rail with neighbours
        let prev_item = vi.checked_sub(1).map(|j| &items[visible[j]]);
        let next_item = visible.get(vi + 1).map(|&j| &items[j]);
        let prev_chain = prev_item.map(is_chain_item).unwrap_or(false);
        let next_chain = next_item.map(is_chain_item).unwrap_or(false);
        let in_chain = is_chain_item(item);

        let item_id = match item {
            TimelineItem::UserMessage { id, .. }
            | TimelineItem::AssistantMessage { id, .. }
            | TimelineItem::Thought { id, .. }
            | TimelineItem::Tool { id, .. }
            | TimelineItem::Plan { id, .. }
            | TimelineItem::Status { id, .. } => id.as_str(),
        };
        let want_scroll = scroll_to_item_id == Some(item_id);
        ui.push_id(timeline_item_salt(item, idx), |ui| {
            let asst_override = match item {
                TimelineItem::AssistantMessage { id, streaming, .. }
                    if *streaming || open_assistant_id == Some(id.as_str()) =>
                {
                    display_assistant
                }
                _ => None,
            };
            // Highlight target user message while jumping
            if want_scroll && matches!(item, TimelineItem::UserMessage { .. }) {
                let fill = if theme::is_dark() {
                    Color32::from_rgba_unmultiplied(96, 165, 250, 22)
                } else {
                    Color32::from_rgba_unmultiplied(37, 99, 235, 18)
                };
                Frame::NONE
                    .fill(fill)
                    .corner_radius(12)
                    .inner_margin(Margin::symmetric(6, 6))
                    .show(ui, |ui| {
                        render_item(
                            ui,
                            w - 12.0,
                            item,
                            md_cache,
                            &mut on_image,
                            &mut resolve_tex,
                            asst_override,
                            expand_tools,
                            in_chain,
                            prev_chain,
                            next_chain,
                            identity,
                        );
                    });
            } else {
                render_item(
                    ui,
                    w,
                    item,
                    md_cache,
                    &mut on_image,
                    &mut resolve_tex,
                    asst_override,
                    expand_tools,
                    in_chain,
                    prev_chain,
                    next_chain,
                    identity,
                );
            }
            if want_scroll {
                // Align message near top of scroll area (Cursor turn nav)
                ui.scroll_to_cursor(Some(Align::TOP));
            }
        });
        let gap = match item {
            TimelineItem::Tool { .. } => 2.0,
            TimelineItem::Thought { .. } => 4.0,
            TimelineItem::Status { .. } => 2.0,
            TimelineItem::UserMessage { .. } => 18.0,
            TimelineItem::AssistantMessage { .. } => 14.0,
            TimelineItem::Plan { .. } => 10.0,
        };
        // Tighter gap between consecutive chain items
        let gap = if in_chain && next_chain { 1.5 } else { gap };
        ui.add_space(gap);
    }
}

/// Tool / status / thought sit on the activity chain (between user ↔ assistant).
fn is_chain_item(item: &TimelineItem) -> bool {
    matches!(
        item,
        TimelineItem::Tool { .. } | TimelineItem::Status { .. } | TimelineItem::Thought { .. }
    )
}

fn timeline_item_salt(item: &TimelineItem, idx: usize) -> egui::Id {
    let kind = match item {
        TimelineItem::UserMessage { .. } => "u",
        TimelineItem::AssistantMessage { .. } => "a",
        TimelineItem::Thought { .. } => "th",
        TimelineItem::Tool { .. } => "t",
        TimelineItem::Plan { .. } => "p",
        TimelineItem::Status { .. } => "s",
    };
    let id = match item {
        TimelineItem::UserMessage { id, .. }
        | TimelineItem::AssistantMessage { id, .. }
        | TimelineItem::Thought { id, .. }
        | TimelineItem::Tool { id, .. }
        | TimelineItem::Plan { id, .. }
        | TimelineItem::Status { id, .. } => id.as_str(),
    };
    egui::Id::new(("c", kind, id, idx))
}

fn render_empty(ui: &mut Ui, w: f32, on_suggestion: &mut impl FnMut(String)) {
    let s = crate::i18n::t();
    ui.add_space(theme::SPACE_XL + 8.0);
    ui.vertical_centered(|ui| {
        icons::grok_logo(ui, 48.0);
        ui.add_space(theme::SPACE_LG);
        ui.label(
            RichText::new(s.empty_title)
                .size(22.0)
                .strong()
                .color(theme::TEXT()),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(s.empty_subtitle)
                .size(13.5)
                .color(theme::TEXT_2()),
        );
    });
    ui.add_space(theme::SPACE_XL);

    // (title, hint, prompt)
    let chips: [(&str, &str, &str); 4] = [
        (s.chip_explore, s.chip_explore_hint, s.chip_explore_prompt),
        (s.chip_fix, s.chip_fix_hint, s.chip_fix_prompt),
        (
            s.chip_implement,
            s.chip_implement_hint,
            s.chip_implement_prompt,
        ),
        (s.chip_docs, s.chip_docs_hint, s.chip_docs_prompt),
    ];
    let row_w = ui.available_width().min(w).min(480.0).max(280.0);
    let outer = ui.available_width();
    let side_pad = ((outer - row_w) * 0.5).max(0.0);
    let gap = 10.0;
    let half = ((row_w - gap) * 0.5).max(130.0);
    let chip_h = 52.0;

    ui.horizontal(|ui| {
        ui.add_space(side_pad);
        ui.vertical(|ui| {
            ui.set_width(row_w);
            for pair in chips.chunks(2) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (title, hint, prompt) in pair {
                        let p = theme::t();
                        let fill = if theme::is_dark() {
                            p.surface
                        } else {
                            Color32::WHITE
                        };
                        let border = if theme::is_dark() {
                            Color32::from_rgba_unmultiplied(255, 255, 255, 14)
                        } else {
                            Color32::from_rgb(0xE6, 0xE6, 0xEA)
                        };
                        let resp = Frame::NONE
                            .fill(fill)
                            .stroke(Stroke::new(1.0, border))
                            .shadow(theme::card_shadow())
                            .corner_radius(12)
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.set_min_size(Vec2::new(half, chip_h));
                                ui.set_max_width(half);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(*title)
                                            .size(13.5)
                                            .strong()
                                            .color(p.text),
                                    );
                                    ui.label(
                                        RichText::new(*hint).size(11.5).color(p.text_3),
                                    );
                                });
                            })
                            .response
                            .interact(Sense::click());
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            ui.painter().rect_stroke(
                                resp.rect,
                                12.0,
                                Stroke::new(1.0, p.accent.gamma_multiply(0.55)),
                                egui::StrokeKind::Outside,
                            );
                        }
                        if resp.clicked() {
                            on_suggestion((*prompt).to_string());
                        }
                    }
                });
                ui.add_space(gap);
            }
        });
    });
}

fn render_chat_image(
    ui: &mut Ui,
    img: &ChatImage,
    max_side: f32,
    on_image: &mut impl FnMut(ChatImage),
    resolve_tex: &mut impl FnMut(&ChatImage) -> Option<TextureHandle>,
) {
    let scale = (max_side / img.width.max(img.height) as f32).min(1.0);
    let size = Vec2::new(img.width as f32 * scale, img.height as f32 * scale).max(Vec2::splat(24.0));

    if let Some(tex) = resolve_tex(img) {
        let resp = ui.add(
            egui::Image::new((tex.id(), size))
                .corner_radius(10.0)
                .sense(Sense::click()),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(
                resp.rect.expand(1.0),
                10.0,
                Stroke::new(1.5, theme::ACCENT()),
                egui::StrokeKind::Outside,
            );
        }
        if resp.clicked() {
            on_image(img.clone());
        }
        resp.on_hover_text(format!("{} — 点击预览", img.label));
    } else if ui
        .add(
            egui::Button::new(RichText::new(&img.label).size(12.0).color(theme::TEXT_2()))
                .fill(theme::SURFACE())
                .corner_radius(6),
        )
        .clicked()
    {
        on_image(img.clone());
    }
}

fn render_item(
    ui: &mut Ui,
    col_w: f32,
    item: &TimelineItem,
    md_cache: &mut CommonMarkCache,
    on_image: &mut impl FnMut(ChatImage),
    resolve_tex: &mut impl FnMut(&ChatImage) -> Option<TextureHandle>,
    display_override: Option<&str>,
    expand_tools: bool,
    in_chain: bool,
    prev_chain: bool,
    next_chain: bool,
    identity: ChatIdentity<'_>,
) {
    let p = theme::t();
    // Full reading width for prose (Cursor); tools share same left edge
    let prose_w = col_w.max(160.0);

    match item {
        TimelineItem::UserMessage {
            text,
            attachments,
            ..
        } => {
            // Right-hug layout: measure bubble width, spacer on the left.
            // (RTL + Align::Min used to dump short text like "行" to the far left.)
            let bubble_max = (col_w * 0.72).clamp(160.0, 560.0);
            let on_bubble = theme::on_user_bubble();
            let letter = AppConfig::avatar_letter(identity.user_name);
            let avatar_path = if identity.user_avatar_path.trim().is_empty() {
                None
            } else {
                Some(identity.user_avatar_path)
            };
            let avatar_sz = 32.0;
            let gap = 10.0;
            let pad_x = 28.0; // horizontal padding inside bubble
            let font = egui::FontId::proportional(14.0);
            let text_w = if text.is_empty() {
                0.0
            } else {
                ui.fonts(|f| {
                    f.layout(
                        text.to_string(),
                        font,
                        on_bubble,
                        (bubble_max - pad_x).max(40.0),
                    )
                    .size()
                    .x
                })
            };
            let attach_w = if attachments.is_empty() {
                0.0
            } else {
                148.0
            };
            let content_w = text_w.max(attach_w).max(28.0);
            let bubble_w = (content_w + pad_x).clamp(44.0, bubble_max);
            // Action chip ~52 wide; keep column at least that wide
            let col_right = bubble_w.max(56.0);
            let row_w = col_right + gap + avatar_sz;
            let left_pad = (col_w - row_w).max(0.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.add_space(left_pad);
                ui.vertical(|ui| {
                    ui.set_width(col_right);
                    // Bubble flush to the right of this column
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        Frame::NONE
                            .fill(p.user_bubble)
                            .stroke(Stroke::new(1.0, p.user_bubble_border))
                            .shadow(if theme::is_dark() {
                                egui::Shadow {
                                    offset: [0, 2],
                                    blur: 8,
                                    spread: 0,
                                    color: Color32::from_black_alpha(50),
                                }
                            } else {
                                theme::card_shadow()
                            })
                            .inner_margin(Margin::symmetric(14, 10))
                            .corner_radius(egui::CornerRadius {
                                nw: 16,
                                ne: 4,
                                sw: 16,
                                se: 16,
                            })
                            .show(ui, |ui| {
                                ui.set_max_width(bubble_w);
                                ui.set_min_width((bubble_w - 4.0).max(32.0));
                                if !attachments.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        for img in attachments {
                                            ui.push_id(&img.id, |ui| {
                                                render_chat_image(
                                                    ui, img, 140.0, on_image, resolve_tex,
                                                );
                                            });
                                            ui.add_space(4.0);
                                        }
                                    });
                                    if !text.is_empty() {
                                        ui.add_space(4.0);
                                    }
                                }
                                if !text.is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(text)
                                                .size(14.0)
                                                .color(on_bubble),
                                        )
                                        .wrap()
                                        .selectable(true),
                                    );
                                }
                            });
                    });
                });
                ui.scope(|ui| {
                    icons::user_avatar_ex(ui, avatar_sz, &letter, avatar_path);
                })
                .response
                .on_hover_text(identity.user_name);
            });
        }

        TimelineItem::AssistantMessage {
            text, streaming, ..
        } => {
            let shown = match display_override {
                Some(d) if !d.is_empty() => {
                    if text.starts_with(d) {
                        d
                    } else if d.starts_with(text.as_str()) && !text.is_empty() {
                        text.as_str()
                    } else {
                        text.as_str()
                    }
                }
                _ => text.as_str(),
            };

            // Avatar + named bubble panel (must contrast stage in dark mode)
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                icons::grok_avatar(ui, 32.0);
                ui.vertical(|ui| {
                    let body_max = (prose_w - 42.0).max(140.0);
                    ui.set_max_width(body_max);

                    Frame::NONE
                        .fill(theme::assistant_panel())
                        .stroke(Stroke::new(1.0, theme::assistant_panel_border()))
                        .shadow(if theme::is_dark() {
                            egui::Shadow {
                                offset: [0, 2],
                                blur: 10,
                                spread: 0,
                                color: Color32::from_black_alpha(55),
                            }
                        } else {
                            theme::card_shadow()
                        })
                        .inner_margin(Margin::symmetric(14, 11))
                        .corner_radius(egui::CornerRadius {
                            nw: 4,
                            ne: 16,
                            sw: 16,
                            se: 16,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(body_max - 4.0);
                            // Header inside bubble
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    RichText::new("Grok")
                                        .size(12.0)
                                        .strong()
                                        .color(p.text_2),
                                );
                                if *streaming {
                                    ui.add(egui::Spinner::new().size(10.0).color(p.accent));
                                    ui.label(
                                        RichText::new(crate::i18n::t().generating)
                                            .size(11.0)
                                            .color(p.accent),
                                    );
                                }
                            });
                            ui.add_space(6.0);

                            if shown.is_empty() && *streaming {
                                ui.label(
                                    RichText::new(crate::i18n::t().organizing_reply)
                                        .size(13.5)
                                        .color(p.text_3),
                                );
                            } else if !shown.is_empty() {
                                let md_src = if *streaming { shown } else { text.as_str() };
                                let inner_w = ui.available_width().max(120.0);
                                ui.style_mut().visuals.override_text_color = Some(p.text);
                                render_assistant_markdown(ui, md_cache, md_src, inner_w);
                                if *streaming {
                                    ui.label(
                                        RichText::new("▍")
                                            .size(13.0)
                                            .color(p.accent)
                                            .strong(),
                                    );
                                }
                            }

                            if !text.is_empty() && !*streaming {
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if message_action_btn(ui, crate::i18n::t().copy).clicked() {
                                        let ok = attachments::set_clipboard_text(text);
                                        ui.ctx().copy_text(text.clone());
                                        if !ok {
                                            ui.ctx().copy_text(text.clone());
                                        }
                                    }
                                });
                            }
                        });
                });
            });
        }

        TimelineItem::Thought { id, text, .. } => {
            let n = text.chars().count();
            let s = crate::i18n::t();
            let label = if n > 0 {
                format!("{} · {n}", s.thought_n)
            } else {
                s.thinking.into()
            };
            // Muted one-line collapse — no purple card chrome
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(
                    RichText::new(label).size(12.0).color(p.text_3),
                )
                .id_salt(("thought", id.as_str()))
                .default_open(false)
                .show(ui, |ui| {
                    ui.set_max_width((col_w - 24.0).max(80.0));
                    ui.add(
                        egui::Label::new(RichText::new(text).size(12.5).color(p.text_2))
                            .wrap()
                            .selectable(true),
                    );
                });
            });
        }

        TimelineItem::Tool {
            id,
            title,
            kind,
            status,
            detail,
            ..
        } => {
            let st = status.to_ascii_lowercase();
            let running = matches!(st.as_str(), "in_progress" | "running" | "pending");
            let failed = matches!(st.as_str(), "failed" | "error");
            let cancelled = matches!(st.as_str(), "cancelled" | "canceled");
            let _ = kind;
            let label = title.clone();
            let s = crate::i18n::t();
            let (st_label, color) = if running {
                (s.tool_running, p.warning)
            } else if failed {
                (s.tool_failed, p.danger)
            } else if cancelled {
                (s.tool_cancelled, p.text_3)
            } else {
                (s.tool_done, p.success)
            };

            // Flat activity line — no nested frames fighting the rail
            with_chain_rail(ui, col_w, in_chain, prev_chain, next_chain, if running {
                color
            } else if failed {
                p.danger
            } else {
                p.border
            }, |ui| {
                let show_detail =
                    !detail.trim().is_empty() && (expand_tools || failed || running);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if running {
                        ui.add(egui::Spinner::new().size(11.0).color(color));
                    } else if failed {
                        ui.label(RichText::new("x").size(12.0).strong().color(color));
                    } else {
                        ui.label(RichText::new("·").size(14.0).color(p.text_3));
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(&label)
                                .size(12.5)
                                .monospace()
                                .color(if running || failed {
                                    p.text
                                } else {
                                    p.text_3
                                }),
                        )
                        .truncate(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(st_label)
                                .size(11.0)
                                .color(color),
                        );
                    });
                });
                if show_detail {
                    ui.add_space(2.0);
                    egui::CollapsingHeader::new(
                        RichText::new(crate::i18n::t().detail)
                            .size(11.0)
                            .color(p.text_3),
                    )
                    .id_salt(("tool_d", id.as_str()))
                    .default_open(failed)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(detail)
                                    .size(11.0)
                                    .monospace()
                                    .color(p.text_2),
                            )
                            .wrap()
                            .selectable(true),
                        );
                    });
                }
            });
        }

        TimelineItem::Plan { entries, .. } => {
            let done = entries
                .iter()
                .filter(|e| e.status == "completed")
                .count();
            let total = entries.len().max(1);
            let any_run = entries.iter().any(|e| e.status == "in_progress");
            Frame::NONE
                .fill(if theme::is_dark() {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 6)
                } else {
                    Color32::from_rgb(0xF5, 0xF5, 0xF7)
                })
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(12, 8))
                .corner_radius(8)
                .show(ui, |ui| {
                    ui.set_max_width(col_w);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if any_run {
                            ui.add(egui::Spinner::new().size(11.0).color(p.warning));
                        }
                        ui.label(
                            RichText::new(crate::i18n::t().plan)
                                .size(12.0)
                                .strong()
                                .color(p.text_2),
                        );
                        ui.label(
                            RichText::new(format!("{done}/{total}"))
                                .size(11.0)
                                .monospace()
                                .color(p.text_3),
                        );
                    });
                    ui.add_space(4.0);
                    for e in entries {
                        let (m, c) = match e.status.as_str() {
                            "completed" => ("+", p.success),
                            "in_progress" => (">", p.warning),
                            _ => ("·", p.text_3),
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(RichText::new(m).size(12.0).monospace().color(c));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&e.content).size(12.5).color(p.text),
                                )
                                .wrap(),
                            );
                        });
                    }
                });
        }

        TimelineItem::Status { text, .. } => {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(text).size(11.5).color(p.text_3));
            });
        }
    }
}

/// Render assistant markdown with proper GFM tables (commonmark tables look broken).
fn render_assistant_markdown(
    ui: &mut Ui,
    md_cache: &mut CommonMarkCache,
    src: &str,
    max_w: f32,
) {
    let chunks = split_md_tables(src);
    if chunks.is_empty() {
        CommonMarkViewer::new()
            .max_image_width(Some(max_w as usize))
            .show(ui, md_cache, src);
        return;
    }
    for (i, chunk) in chunks.iter().enumerate() {
        ui.push_id(("mdc", i), |ui| match chunk {
            MdChunk::Prose(s) => {
                if !s.trim().is_empty() {
                    CommonMarkViewer::new()
                        .max_image_width(Some(max_w as usize))
                        .show(ui, md_cache, s);
                }
            }
            MdChunk::Table { headers, rows } => {
                ui.add_space(6.0);
                render_md_table(ui, headers, rows, max_w);
                ui.add_space(8.0);
            }
        });
    }
}

enum MdChunk {
    Prose(String),
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

fn split_md_tables(src: &str) -> Vec<MdChunk> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if is_table_row(lines[i])
            && i + 1 < lines.len()
            && is_table_sep(lines[i + 1])
        {
            if !prose.is_empty() {
                out.push(MdChunk::Prose(prose.join("\n")));
                prose.clear();
            }
            let headers = parse_table_row(lines[i]);
            i += 2; // skip header + separator
            let mut rows = Vec::new();
            while i < lines.len() && is_table_row(lines[i]) {
                rows.push(parse_table_row(lines[i]));
                i += 1;
            }
            out.push(MdChunk::Table { headers, rows });
            continue;
        }
        prose.push(lines[i]);
        i += 1;
    }
    if !prose.is_empty() {
        out.push(MdChunk::Prose(prose.join("\n")));
    }
    out
}

fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.matches('|').count() >= 2
}

fn is_table_sep(line: &str) -> bool {
    let t = line.trim();
    if !t.starts_with('|') {
        return false;
    }
    t.chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
        && t.contains('-')
}

fn parse_table_row(line: &str) -> Vec<String> {
    let t = line.trim().trim_matches('|');
    t.split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn render_md_table(ui: &mut Ui, headers: &[String], rows: &[Vec<String>], max_w: f32) {
    let p = theme::t();
    let cols = headers.len().max(1);
    let col_w = ((max_w - 2.0) / cols as f32).max(72.0);

    Frame::NONE
        .fill(if theme::is_dark() {
            p.surface
        } else {
            Color32::WHITE
        })
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(8)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_max_width(max_w);
            // Header
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for h in headers {
                    Frame::NONE
                        .fill(if theme::is_dark() {
                            p.surface_2
                        } else {
                            Color32::from_rgb(0xF4, 0xF4, 0xF5)
                        })
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_min_width(col_w - 1.0);
                            ui.set_max_width(col_w - 1.0);
                            ui.label(
                                RichText::new(h)
                                    .size(12.5)
                                    .strong()
                                    .color(p.text),
                            );
                        });
                }
            });
            // Hairline under header
            let r = ui.min_rect();
            ui.painter().hline(
                r.x_range(),
                r.bottom(),
                Stroke::new(1.0, p.border),
            );
            for (ri, row) in rows.iter().enumerate() {
                let bg = if ri % 2 == 1 {
                    if theme::is_dark() {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 6)
                    } else {
                        Color32::from_rgb(0xFA, 0xFA, 0xFB)
                    }
                } else {
                    Color32::TRANSPARENT
                };
                Frame::NONE
                    .fill(bg)
                    .inner_margin(Margin::ZERO)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for ci in 0..cols {
                                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                                Frame::NONE
                                    .inner_margin(Margin::symmetric(10, 7))
                                    .show(ui, |ui| {
                                        ui.set_min_width(col_w - 1.0);
                                        ui.set_max_width(col_w - 1.0);
                                        // path-like cells mono
                                        let mono = cell.contains('/')
                                            || cell.contains('\\')
                                            || cell.ends_with(".rs")
                                            || cell.ends_with('/');
                                        let rt = if mono {
                                            RichText::new(cell)
                                                .size(12.5)
                                                .monospace()
                                                .color(p.text)
                                        } else {
                                            RichText::new(cell).size(13.0).color(p.text_2)
                                        };
                                        ui.add(egui::Label::new(rt).wrap());
                                    });
                            }
                        });
                    });
                if ri + 1 < rows.len() {
                    let r = ui.min_rect();
                    ui.painter().hline(
                        r.x_range(),
                        r.bottom(),
                        Stroke::new(
                            1.0,
                            if theme::is_dark() {
                                Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                            } else {
                                Color32::from_rgb(0xEE, 0xEE, 0xF0)
                            },
                        ),
                    );
                }
            }
        });
}

/// Action chip under assistant panel.
fn message_action_btn(ui: &mut Ui, label: &str) -> egui::Response {
    let p = theme::t();
    let resp = ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(p.text_2))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(6)
            .min_size(Vec2::new(44.0, 24.0))
            .sense(Sense::click()),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(
            resp.rect,
            6.0,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 14)
            } else {
                Color32::from_black_alpha(12)
            },
        );
    }
    resp
}

/// Thin left spine for tool / thought / status chain (aligned with prose rail).
fn with_chain_rail(
    ui: &mut Ui,
    col_w: f32,
    in_chain: bool,
    prev_chain: bool,
    next_chain: bool,
    rail_color: Color32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let rail_w = 10.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        // Match assistant left rail gutter
        ui.add_space(theme::CHAT_RAIL + 12.0);
        if in_chain {
            let (rail_rect, _) = ui.allocate_exact_size(Vec2::new(rail_w, 1.0), Sense::hover());
            let content_w = (col_w - theme::CHAT_RAIL - 12.0 - rail_w).max(60.0);
            let resp = ui
                .allocate_ui_with_layout(
                    Vec2::new(content_w, ui.available_height()),
                    Layout::top_down(Align::LEFT),
                    |ui| {
                        ui.set_width(content_w);
                        ui.set_max_width(content_w);
                        add_contents(ui);
                    },
                )
                .response;
            let x = rail_rect.center().x;
            let top = resp.rect.top();
            let bot = resp.rect.bottom();
            let mid_y = resp.rect.center().y;
            let painter = ui.painter();
            let line = Stroke::new(1.25, rail_color);
            if prev_chain {
                painter.line_segment([egui::pos2(x, top), egui::pos2(x, mid_y - 3.5)], line);
            }
            if next_chain {
                painter.line_segment([egui::pos2(x, mid_y + 3.5), egui::pos2(x, bot)], line);
            }
            painter.circle_filled(egui::pos2(x, mid_y), 2.5, rail_color);
        } else {
            let content_w = (col_w - theme::CHAT_RAIL - 12.0).max(60.0);
            ui.allocate_ui_with_layout(
                Vec2::new(content_w, ui.available_height()),
                Layout::top_down(Align::LEFT),
                |ui| {
                    ui.set_max_width(content_w);
                    add_contents(ui);
                },
            );
        }
    });
}


