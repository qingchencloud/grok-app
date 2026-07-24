//! Shared controls — one size scale from `theme` (BTN_H_* / SPACE_*).

use super::icons::{self, IconKind};
use super::theme;
use egui::{
    Align, Color32, Frame, Layout, Margin, Response, RichText, Sense, Stroke, Ui, UiBuilder, Vec2,
};

pub fn centered_column(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui)) {
    let avail = ui.available_width();
    let width = avail.min(max_width).max(240.0);
    let pad = ((avail - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        if pad > 1.0 {
            ui.add_space(pad);
        }
        ui.vertical(|ui| {
            ui.set_width(width);
            ui.set_max_width(width);
            add(ui);
        });
    });
}

pub fn card(ui: &mut Ui, fill: Color32, max_w: f32, add: impl FnOnce(&mut Ui)) {
    let w = ui.available_width().min(max_w).max(40.0);
    Frame::NONE
        .fill(fill)
        .inner_margin(Margin::symmetric(theme::SPACE_MD as i8, theme::SPACE_MD as i8 - 2))
        .corner_radius(theme::RADIUS_LG)
        .show(ui, |ui| {
            ui.set_width(w.min(ui.available_width()));
            ui.set_max_width(w);
            add(ui);
        });
}

pub fn avatar(ui: &mut Ui, letter: &str, fill: Color32, text_color: Color32) {
    let size = 26.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .circle_filled(rect.center(), size * 0.5, fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(11.5),
            text_color,
        );
    }
}

pub fn status_dot(ui: &mut Ui, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter().circle_filled(rect.center(), 3.5, color);
        }
        ui.label(RichText::new(label).size(12.5).color(theme::TEXT_2()));
    });
}

/// Primary CTA — send / confirm. Height BTN_H_LG, min width ~80.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let p = theme::t();
    let btn = egui::Button::new(
        RichText::new(text)
            .strong()
            .color(if enabled { p.on_accent } else { p.text_3 })
            .size(13.5),
    )
    .fill(if enabled { p.send_btn } else { p.surface_2 })
    .stroke(Stroke::NONE)
    .corner_radius(theme::RADIUS_PILL)
    .min_size(Vec2::new(80.0, theme::BTN_H_LG));
    let resp = ui.add_enabled(enabled, btn);
    if resp.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Secondary / ghost — outlined chip, BTN_H_MD.
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    let p = theme::t();
    let resp = ui.add(
        egui::Button::new(RichText::new(text).size(13.0).color(p.accent))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(52.0, theme::BTN_H_MD)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Low-emphasis text action (import / archive) — not a primary CTA.
pub fn quiet_link(ui: &mut Ui, text: &str) -> Response {
    let p = theme::t();
    // Two-pass color: allocate with sense, paint with hover state
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(11.5),
            Color32::WHITE,
        )
    });
    let size = Vec2::new(galley.size().x + 4.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let c = if resp.hovered() { p.text_2 } else { p.text_3 };
        ui.painter().text(
            rect.left_center() + egui::vec2(2.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(11.5),
            c,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Compact search field for sidebar / modals.
pub fn search_field(ui: &mut Ui, id: impl std::hash::Hash, text: &mut String, hint: &str) -> Response {
    let p = theme::t();
    let w = ui.available_width().max(40.0);
    Frame::NONE
        .fill(if theme::is_dark() {
            p.surface
        } else {
            Color32::from_rgb(0xF5, 0xF5, 0xF7)
        })
        .stroke(Stroke::new(
            1.0,
            if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            } else {
                Color32::from_rgb(0xE4, 0xE4, 0xE8)
            },
        ))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(w - 4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new("⌕").size(13.0).color(p.text_3));
                let te = egui::TextEdit::singleline(text)
                    .id_salt(id)
                    .desired_width(ui.available_width() - 28.0)
                    .hint_text(RichText::new(hint).size(12.5).color(p.text_3))
                    .frame(false)
                    .text_color(p.text);
                let r = ui.add(te);
                if !text.is_empty() {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(13.0).color(p.text_3))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::splat(18.0)),
                        )
                        .clicked()
                    {
                        text.clear();
                    }
                }
                r
            })
            .inner
        })
        .inner
}

/// Soft secondary button for modals (import row) — not loud primary pill.
pub fn soft_action(ui: &mut Ui, text: &str) -> Response {
    let p = theme::t();
    let resp = ui.add(
        egui::Button::new(RichText::new(text).size(12.0).color(p.text_2))
            .fill(if theme::is_dark() {
                Color32::from_rgba_unmultiplied(255, 255, 255, 8)
            } else {
                Color32::from_rgb(0xF0, 0xF0, 0xF3)
            })
            .stroke(Stroke::new(
                1.0,
                if theme::is_dark() {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 14)
                } else {
                    Color32::from_rgb(0xE4, 0xE4, 0xE8)
                },
            ))
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(52.0, 26.0)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Compact context-window meter for the topbar.
/// `used` / `max` tokens; when `max` is 0 the bar is empty and shows `used` only.
pub fn context_meter(ui: &mut Ui, used: u64, max: Option<u64>, tip: &str) -> Response {
    let p = theme::t();
    let max_v = max.unwrap_or(0).max(1);
    let frac = if max.is_some() {
        (used as f32 / max_v as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Color shifts as the window fills
    let fill_c = if frac >= 0.90 {
        p.danger
    } else if frac >= 0.70 {
        p.warning
    } else {
        p.accent
    };
    let label = match max {
        Some(m) => format!(
            "{}/{}",
            format_tokens_short(used),
            format_tokens_short(m.max(1))
        ),
        None => format!("{}·?", format_tokens_short(used)),
    };

    let bar_w = 72.0;
    let bar_h = 5.0;
    let row_h = 22.0;
    // Estimate total width: label + gap + bar
    let font = egui::FontId::proportional(11.0);
    let label_w = ui.fonts(|f| {
        f.layout_no_wrap(label.clone(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    let total_w = label_w + 8.0 + bar_w + 4.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(total_w, row_h), Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // Label on the left
        painter.text(
            egui::pos2(rect.left(), rect.center().y),
            egui::Align2::LEFT_CENTER,
            &label,
            font,
            p.text_3,
        );
        // Track
        let track = egui::Rect::from_min_size(
            egui::pos2(rect.right() - bar_w, rect.center().y - bar_h * 0.5),
            Vec2::new(bar_w, bar_h),
        );
        let track_fill = if theme::is_dark() {
            Color32::from_rgba_unmultiplied(255, 255, 255, 18)
        } else {
            Color32::from_rgb(0xE4, 0xE4, 0xE8)
        };
        painter.rect_filled(track, 3.0, track_fill);
        if frac > 0.001 {
            let mut filled = track;
            filled.set_width((track.width() * frac).max(2.0));
            painter.rect_filled(filled, 3.0, fill_c);
        }
    }
    resp.on_hover_text(tip)
}

fn format_tokens_short(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}k", n / 1000)
    } else if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Ghost text button for topbar actions — no border, soft hover only.
pub fn chip_button(ui: &mut Ui, text: &str) -> Response {
    let p = theme::t();
    let resp = ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(p.text_2))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(theme::RADIUS_SM)
            .min_size(Vec2::new(40.0, theme::BTN_H_SM)),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Status pill without chrome: colored dot + label only.
pub fn status_pill(ui: &mut Ui, color: Color32, label: &str, spinning: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if spinning {
            ui.add(egui::Spinner::new().size(10.0));
        } else {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
            if ui.is_rect_visible(r) {
                ui.painter().circle_filled(r.center(), 3.0, color);
            }
        }
        ui.label(RichText::new(label).size(12.0).color(theme::TEXT_2()));
    });
}

pub fn suggestion_chip(ui: &mut Ui, size: Vec2, icon: &str, title: &str, subtitle: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if resp.hovered() {
        theme::HOVER()
    } else {
        theme::SURFACE()
    };
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, theme::RADIUS as f32, fill);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect.shrink2(Vec2::new(12.0, 10.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        child.horizontal(|ui| {
            ui.label(RichText::new(icon).size(15.0));
            ui.add_space(theme::SPACE_SM);
            ui.vertical(|ui| {
                ui.set_max_width((size.x - 40.0).max(40.0));
                ui.label(RichText::new(title).size(13.5).strong().color(theme::TEXT()));
                ui.label(RichText::new(subtitle).size(11.5).color(theme::TEXT_3()));
            });
        });
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Live activity kind for a session row (sidebar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionActivity {
    /// Idle / not the live session.
    Idle,
    /// Currently selected but not generating.
    Current,
    /// Model streaming a reply.
    Generating,
    /// Tool call in progress.
    Tool,
    /// Waiting for permission modal.
    Permission,
    /// Connecting / reconnecting agent.
    Connecting,
}

impl SessionActivity {
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Idle | Self::Current)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Current => crate::i18n::t().act_current,
            Self::Generating => crate::i18n::t().act_generating,
            Self::Tool => crate::i18n::t().act_tool,
            Self::Permission => crate::i18n::t().act_permission,
            Self::Connecting => crate::i18n::t().act_connecting,
        }
    }
}

/// Single-line elide by pixel width (no wrap — keeps list row heights stable).
pub fn elide_text(ui: &Ui, text: &str, font: egui::FontId, max_w: f32) -> String {
    if max_w <= 8.0 {
        return "…".into();
    }
    let full = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE));
    if full.size().x <= max_w {
        return text.to_string();
    }
    // Binary-search char count so we fit with ellipsis
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        let g = ui.fonts(|f| f.layout_no_wrap(candidate, font.clone(), Color32::WHITE));
        if g.size().x <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        "…".into()
    } else {
        chars[..lo].iter().collect::<String>() + "…"
    }
}

/// Session row — fixed height, single-line title + optional right status chip.
/// Leading spinner when busy (works even if row is not selected).
pub fn session_row(
    ui: &mut Ui,
    title: &str,
    meta: Option<&str>,
    selected: bool,
    activity: SessionActivity,
) -> Response {
    let width = ui.available_width().max(40.0);
    let height = theme::SESSION_ROW_H;
    let busy = activity.is_active();
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, height),
        Sense::click().union(Sense::hover()),
    );

    let p = theme::t();
    let bg = if selected || busy {
        p.selected
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };

    if ui.is_rect_visible(rect) {
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 8.0, bg);
        }
        if selected || busy {
            let bar = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 1.0, rect.top() + 7.0),
                egui::pos2(rect.left() + 3.0, rect.bottom() - 7.0),
            );
            let bar_c = match activity {
                SessionActivity::Permission => p.warning,
                _ if busy => p.accent,
                _ => p.accent.gamma_multiply(0.65),
            };
            ui.painter().rect_filled(bar, 1.5, bar_c);
        }

        // Leading status
        let lead_c = egui::pos2(rect.left() + 14.0, rect.center().y);
        let lead_rect = egui::Rect::from_center_size(lead_c, Vec2::splat(16.0));
        match activity {
            SessionActivity::Generating
            | SessionActivity::Tool
            | SessionActivity::Connecting
            | SessionActivity::Permission => {
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(lead_rect)
                        .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
                );
                let sp_color = if activity == SessionActivity::Permission {
                    p.warning
                } else {
                    p.accent
                };
                child.add(egui::Spinner::new().size(12.0).color(sp_color));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(16));
            }
            SessionActivity::Current => {
                ui.painter().circle_filled(lead_c, 3.5, p.accent);
            }
            SessionActivity::Idle => {
                ui.painter()
                    .circle_stroke(lead_c, 3.0, Stroke::new(1.0, p.text_3));
            }
        }

        // Right status badge (short, never wraps)
        let badge = if busy {
            Some(activity.label())
        } else {
            None
        };
        let badge_w = if badge.is_some() { 48.0 } else { 0.0 };
        let text_left = rect.left() + 28.0;
        let text_right = rect.right() - 8.0 - badge_w;
        let max_w = (text_right - text_left).max(24.0);

        let title_color = if selected || busy || resp.hovered() {
            p.text
        } else {
            p.text_2
        };
        let font = egui::FontId::proportional(13.0);
        let shown = elide_text(ui, title, font.clone(), max_w);
        ui.painter().text(
            egui::pos2(text_left, rect.center().y),
            egui::Align2::LEFT_CENTER,
            shown,
            font,
            title_color,
        );

        if let Some(b) = badge {
            let bc = match activity {
                SessionActivity::Permission => p.warning,
                _ => p.accent,
            };
            ui.painter().text(
                egui::pos2(rect.right() - 8.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                b,
                egui::FontId::proportional(10.5),
                bc,
            );
        }
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let tip = match (meta, busy) {
        (Some(m), true) if !m.is_empty() => format!("{title}\n{} · {m}", activity.label()),
        (_, true) => format!("{title}\n{}", activity.label()),
        (Some(m), false) if !m.is_empty() => format!("{title}\n{m}"),
        _ => title.to_string(),
    };
    resp.on_hover_text(tip)
}

/// Project row — reference `.tree-l2`: folder + name, expand/collapse only (never selected).
pub fn project_row(
    ui: &mut Ui,
    name: &str,
    count: usize,
    collapsed: bool,
    _is_current: bool,
) -> Response {
    let width = ui.available_width().max(40.0);
    let height = theme::PROJECT_ROW_H;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, height),
        Sense::click().union(Sense::hover()),
    );
    let p = theme::t();
    let bg = if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    if ui.is_rect_visible(rect) {
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 8.0, bg);
        }
        let name_color = if resp.hovered() { p.text } else { p.text_2 };
        let icon_c = if resp.hovered() { p.text } else { p.text_2 };

        // Folder icon only (reference has no chevron on l2)
        let fold_r = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            Vec2::splat(15.0),
        );
        icons::paint_in(ui, IconKind::Folder, fold_r, icon_c);

        // Collapsed hint: faint chevron after name area is overkill — use opacity on count
        let max_w = (rect.width() - 56.0).max(20.0);
        let galley = ui.fonts(|f| {
            f.layout(
                name.to_string(),
                egui::FontId::proportional(13.0),
                name_color,
                max_w,
            )
        });
        ui.painter().galley(
            egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y * 0.5),
            galley,
            name_color,
        );

        // Session count on the right (quiet)
        if count > 0 || collapsed {
            let badge = if collapsed {
                format!("{count}")
            } else {
                String::new()
            };
            if !badge.is_empty() {
                ui.painter().text(
                    egui::pos2(rect.right() - 8.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    badge,
                    egui::FontId::proportional(11.0),
                    p.text_3,
                );
            }
        }
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Section head — reference `.tree-l1__head` (Projects / History).
pub fn tree_section_head(ui: &mut Ui, label: &str, open: bool) -> Response {
    let width = ui.available_width().max(40.0);
    let height = theme::TREE_L1_H;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, height),
        Sense::click().union(Sense::hover()),
    );
    let p = theme::t();
    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            ui.painter().rect_filled(rect, 8.0, p.hover);
        }
        let c = if resp.hovered() { p.text_2 } else { p.text_3 };
        let chev = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            Vec2::splat(12.0),
        );
        icons::paint_in(
            ui,
            if open {
                IconKind::ChevronDown
            } else {
                IconKind::ChevronRightSmall
            },
            chev,
            c,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 26.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            c,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Nav action row (reference `.nav-new`) — vector icon + label, soft hover, no border.
pub fn nav_row(ui: &mut Ui, kind: IconKind, label: &str) -> Response {
    let width = ui.available_width().max(40.0);
    let height = 34.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(width, height),
        Sense::click().union(Sense::hover()),
    );
    let p = theme::t();
    let bg = if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    if ui.is_rect_visible(rect) {
        if bg != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, theme::RADIUS as f32, bg);
        }
        let color = if resp.hovered() { p.text } else { p.text_2 };
        let icon_r = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            Vec2::splat(16.0),
        );
        icons::paint_in(ui, kind, icon_r, color);
        ui.painter().text(
            egui::pos2(rect.left() + 34.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            color,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(theme::SPACE_SM);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .color(theme::TEXT_3())
            .strong(),
    );
    ui.add_space(theme::SPACE_XS);
}

pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    // Legacy text icons still used in a few places — prefer icons::icon_btn.
    ui.add(
        egui::Button::new(RichText::new(icon).size(15.0).color(theme::TEXT_2()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::splat(theme::ICON_BTN)),
    )
    .on_hover_text(tip)
}

pub fn icon_btn(ui: &mut Ui, kind: IconKind, tip: &str) -> Response {
    icons::icon_btn(ui, kind, tip)
}

pub fn banner(ui: &mut Ui, fill: Color32, text: &str, on_close: &mut bool) {
    let w = ui.available_width();
    Frame::NONE
        .fill(fill)
        .inner_margin(Margin::symmetric(12, 8))
        .corner_radius(theme::RADIUS)
        .show(ui, |ui| {
            ui.set_max_width(w);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SPACE_SM;
                ui.add(egui::Label::new(RichText::new(text).size(12.5)).wrap());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(14.0).color(theme::TEXT_3()))
                                .frame(false)
                                .min_size(Vec2::splat(theme::BTN_H_SM)),
                        )
                        .clicked()
                    {
                        *on_close = true;
                    }
                });
            });
        });
}

pub fn path_short(path: &str, max_chars: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max_chars {
        return path.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let tail: String = chars[chars.len().saturating_sub(keep)..].iter().collect();
    format!("…{tail}")
}

pub fn hairline(ui: &mut Ui) {
    let y = ui.cursor().top() + 0.5;
    let rect = ui.max_rect();
    // Inset slightly so hairlines don't butt into the panel edge stroke
    let x0 = rect.left() + 4.0;
    let x1 = rect.right() - 4.0;
    if x1 > x0 {
        ui.painter()
            .hline(x0..=x1, y, theme::separator_stroke());
    }
    ui.add_space(1.0);
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
