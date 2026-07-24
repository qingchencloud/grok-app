//! Design tokens — developer IDE / Cursor aesthetic (ui-ux-pro-max).
//! Light: monochrome + blue accent, high body contrast.
//! Dark: slate code dark + green success.
#![allow(non_snake_case)]

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};
use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(true);

#[inline]
pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub accent_dim: Color32,
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub border: Color32,
    pub divider: Color32,
    pub text: Color32,
    pub text_2: Color32,
    pub text_3: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub purple: Color32,
    pub user_bubble: Color32,
    pub user_bubble_border: Color32,
    pub tool_row: Color32,
    pub code_bg: Color32,
    pub avatar: Color32,
    pub send_btn: Color32,
    pub on_accent: Color32,
    pub ring: Color32,
    pub chip: Color32,
    pub chip_border: Color32,
}

#[inline]
pub fn t() -> Palette {
    if is_dark() {
        DARK_PALETTE
    } else {
        LIGHT_PALETTE
    }
}

// Developer dark (ui-ux-pro-max: Code dark + run green)
const DARK_PALETTE: Palette = Palette {
    accent: Color32::from_rgb(0x60, 0xA5, 0xFA), // blue-400
    accent_dim: Color32::from_rgb(0x3B, 0x82, 0xF6),
    bg: Color32::from_rgb(0x0F, 0x11, 0x15),
    sidebar: Color32::from_rgb(0x13, 0x16, 0x1B),
    surface: Color32::from_rgb(0x18, 0x1B, 0x21),
    surface_2: Color32::from_rgb(0x1E, 0x22, 0x29),
    hover: Color32::from_rgb(0x24, 0x28, 0x30),
    selected: Color32::from_rgb(0x2A, 0x30, 0x3A),
    border: Color32::from_rgb(0x2E, 0x34, 0x3E),
    divider: Color32::from_rgb(0x22, 0x26, 0x2E),
    text: Color32::from_rgb(0xF1, 0xF5, 0xF9), // slate-100 — AA
    text_2: Color32::from_rgb(0x94, 0xA3, 0xB8), // slate-400
    text_3: Color32::from_rgb(0x64, 0x74, 0x8B), // slate-500
    success: Color32::from_rgb(0x22, 0xC5, 0x5E),
    warning: Color32::from_rgb(0xF5, 0x9E, 0x0B),
    danger: Color32::from_rgb(0xEF, 0x44, 0x44),
    purple: Color32::from_rgb(0xA7, 0x8B, 0xFA),
    // Solid blue command bubble (readable on near-black stage)
    user_bubble: Color32::from_rgb(0x25, 0x63, 0xEB),
    user_bubble_border: Color32::from_rgb(0x3B, 0x82, 0xF6),
    tool_row: Color32::from_rgb(0x16, 0x19, 0x20),
    code_bg: Color32::from_rgb(0x0B, 0x0D, 0x11),
    avatar: Color32::from_rgb(0x60, 0xA5, 0xFA),
    send_btn: Color32::from_rgb(0x3B, 0x82, 0xF6),
    on_accent: Color32::WHITE,
    ring: Color32::from_rgb(0x60, 0xA5, 0xFA),
    chip: Color32::from_rgb(0x1E, 0x22, 0x29),
    chip_border: Color32::from_rgb(0x33, 0x41, 0x55),
};

// Linear / Cursor light — monochrome + blue (WCAG body contrast)
const LIGHT_PALETTE: Palette = Palette {
    accent: Color32::from_rgb(0x25, 0x63, 0xEB), // blue-600
    accent_dim: Color32::from_rgb(0x1D, 0x4E, 0xD8),
    bg: Color32::from_rgb(0xF7, 0xF7, 0xF8),
    sidebar: Color32::from_rgb(0xEE, 0xEE, 0xF0),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_2: Color32::from_rgb(0xF0, 0xF0, 0xF3),
    hover: Color32::from_rgb(0xE4, 0xE4, 0xE7),
    selected: Color32::from_rgb(0xDC, 0xDC, 0xE0),
    border: Color32::from_rgb(0xE4, 0xE4, 0xE7),
    divider: Color32::from_rgb(0xEB, 0xEB, 0xEE),
    text: Color32::from_rgb(0x09, 0x09, 0x0B), // zinc-950 — max contrast
    text_2: Color32::from_rgb(0x52, 0x52, 0x5B), // zinc-600
    text_3: Color32::from_rgb(0x71, 0x71, 0x7A), // zinc-500 — still ≥4.5 on white for large
    success: Color32::from_rgb(0x16, 0xA3, 0x4A),
    warning: Color32::from_rgb(0xD9, 0x77, 0x06),
    danger: Color32::from_rgb(0xDC, 0x26, 0x26),
    purple: Color32::from_rgb(0x7C, 0x3A, 0xED),
    // Soft blue command bubble (clear on #F7F7F8 stage)
    user_bubble: Color32::from_rgb(0xDB, 0xE7, 0xFF),
    user_bubble_border: Color32::from_rgb(0xB4, 0xCC, 0xF5),
    tool_row: Color32::from_rgb(0xF3, 0xF4, 0xF6),
    code_bg: Color32::from_rgb(0xF4, 0xF4, 0xF5),
    avatar: Color32::from_rgb(0x25, 0x63, 0xEB),
    send_btn: Color32::from_rgb(0x25, 0x63, 0xEB),
    on_accent: Color32::WHITE,
    ring: Color32::from_rgb(0x3B, 0x82, 0xF6),
    chip: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    chip_border: Color32::from_rgb(0xE4, 0xE4, 0xE7),
};

pub const SIDEBAR_WIDTH: f32 = 248.0;
/// Reading column — tables + code need air but not absurd margins
pub const CHAT_MAX_WIDTH: f32 = 780.0;
pub const EDGE_PAD: f32 = 24.0;
/// Left rail for assistant prose alignment
pub const CHAT_RAIL: f32 = 3.0;
pub const CHAT_MARK: f32 = 16.0;
pub const CHAT_AVATAR: f32 = 24.0;
pub const CHAT_NAME_MAX_W: f32 = 140.0;

pub const COMPOSER_MIN_H: f32 = 44.0;
pub const COMPOSER_PANEL_H: f32 = 148.0;
pub const COMPOSER_THUMB_EXTRA: f32 = 76.0;
pub const COMPOSER_TEXT_H: f32 = 48.0;

pub const RADIUS: u8 = 8;
pub const RADIUS_SM: u8 = 6;
pub const RADIUS_LG: u8 = 10;
pub const RADIUS_PILL: u8 = 100;

pub const BTN_H_SM: f32 = 28.0;
pub const BTN_H_MD: f32 = 32.0;
pub const BTN_H_LG: f32 = 36.0;
pub const BTN_H_XL: f32 = 36.0;
pub const ICON_BTN: f32 = 28.0;
pub const SESSION_ROW_H: f32 = 32.0;
pub const SESSION_ROW_GAP: f32 = 2.0;
pub const PROJECT_ROW_H: f32 = 32.0;
pub const TREE_L1_H: f32 = 28.0;
pub const TOPBAR_H: f32 = 40.0;
pub const TOUCH_MIN: f32 = BTN_H_MD;
pub const TOUCH_CTA: f32 = BTN_H_LG;

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;

#[inline]
pub fn ACCENT() -> Color32 {
    t().accent
}
#[inline]
pub fn ACCENT_DIM() -> Color32 {
    t().accent_dim
}
#[inline]
pub fn BG() -> Color32 {
    t().bg
}
#[inline]
pub fn SIDEBAR() -> Color32 {
    t().sidebar
}
#[inline]
pub fn SURFACE() -> Color32 {
    t().surface
}
#[inline]
pub fn SURFACE_2() -> Color32 {
    t().surface_2
}
#[inline]
pub fn HOVER() -> Color32 {
    t().hover
}
#[inline]
pub fn SELECTED() -> Color32 {
    t().selected
}
#[inline]
pub fn BORDER() -> Color32 {
    t().border
}
#[inline]
pub fn DIVIDER() -> Color32 {
    t().divider
}
#[inline]
pub fn TEXT() -> Color32 {
    t().text
}
#[inline]
pub fn TEXT_2() -> Color32 {
    t().text_2
}
#[inline]
pub fn TEXT_3() -> Color32 {
    t().text_3
}
#[inline]
pub fn SUCCESS() -> Color32 {
    t().success
}
#[inline]
pub fn WARNING() -> Color32 {
    t().warning
}
#[inline]
pub fn DANGER() -> Color32 {
    t().danger
}
#[inline]
pub fn PURPLE() -> Color32 {
    t().purple
}
#[inline]
pub fn USER_BUBBLE() -> Color32 {
    t().user_bubble
}
#[inline]
pub fn USER_BUBBLE_BORDER() -> Color32 {
    t().user_bubble_border
}
#[inline]
pub fn TOOL_ROW() -> Color32 {
    t().tool_row
}
#[inline]
pub fn PANEL() -> Color32 {
    t().sidebar
}
#[inline]
pub fn ELEVATED() -> Color32 {
    t().surface
}
#[inline]
pub fn ELEVATED_2() -> Color32 {
    t().surface_2
}
#[inline]
pub fn BORDER_STRONG() -> Color32 {
    t().border
}
#[inline]
pub fn SEPARATOR() -> Color32 {
    t().divider
}
#[inline]
pub fn TEXT_SECONDARY() -> Color32 {
    t().text
}
#[inline]
pub fn TEXT_MUTED() -> Color32 {
    t().text_2
}
#[inline]
pub fn TEXT_DIM() -> Color32 {
    t().text_3
}
#[inline]
pub fn ON_ACCENT() -> Color32 {
    t().on_accent
}
#[inline]
pub fn RING() -> Color32 {
    t().ring
}
#[inline]
pub fn CHIP() -> Color32 {
    t().chip
}
#[inline]
pub fn CHIP_BORDER() -> Color32 {
    t().chip_border
}
#[inline]
pub fn SEND_BTN() -> Color32 {
    t().send_btn
}
#[inline]
pub fn CODE_BG() -> Color32 {
    t().code_bg
}

pub fn on_user_bubble() -> Color32 {
    if is_dark() {
        // Solid blue bubble → white body text
        Color32::WHITE
    } else {
        Color32::from_rgb(0x0F, 0x17, 0x2A)
    }
}

pub fn separator_stroke() -> Stroke {
    Stroke::new(1.0, t().divider)
}

pub fn sidebar_edge_stroke() -> Stroke {
    if is_dark() {
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 12))
    } else {
        Stroke::new(1.0, Color32::from_black_alpha(12))
    }
}

pub fn paint_sidebar_edge(painter: &egui::Painter, rect: egui::Rect) {
    let x = rect.right() - 0.5;
    painter.vline(x, rect.y_range(), sidebar_edge_stroke());
}

pub fn elev_shadow() -> egui::Shadow {
    if is_dark() {
        egui::Shadow {
            offset: [0, 2],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(48),
        }
    } else {
        egui::Shadow {
            offset: [0, 4],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(18),
        }
    }
}

pub fn card_shadow() -> egui::Shadow {
    if is_dark() {
        egui::Shadow {
            offset: [0, 1],
            blur: 6,
            spread: 0,
            color: Color32::from_black_alpha(40),
        }
    } else {
        egui::Shadow {
            offset: [0, 1],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(14),
        }
    }
}

pub fn modal_shadow() -> egui::Shadow {
    if is_dark() {
        egui::Shadow {
            offset: [0, 8],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(80),
        }
    } else {
        egui::Shadow {
            offset: [0, 12],
            blur: 40,
            spread: 0,
            color: Color32::from_black_alpha(28),
        }
    }
}

pub fn modal_scrim() -> Color32 {
    if is_dark() {
        Color32::from_black_alpha(160)
    } else {
        Color32::from_black_alpha(80)
    }
}

pub fn modal_fill() -> Color32 {
    t().surface
}

pub fn modal_stroke() -> Stroke {
    Stroke::new(1.0, t().border)
}

pub fn modal_nav_fill() -> Color32 {
    if is_dark() {
        t().surface_2
    } else {
        Color32::from_rgb(0xF4, 0xF4, 0xF5)
    }
}

/// Soft success badge fill (tool completed).
pub fn success_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(34, 197, 94, 32)
    } else {
        Color32::from_rgb(0xDC, 0xFC, 0xE7)
    }
}

pub fn warning_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(245, 158, 11, 28)
    } else {
        Color32::from_rgb(0xFE, 0xF3, 0xC7)
    }
}

pub fn danger_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(239, 68, 68, 28)
    } else {
        Color32::from_rgb(0xFE, 0xE2, 0xE2)
    }
}

/// Soft accent wash (assistant surface / hover).
pub fn accent_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(96, 165, 250, 18)
    } else {
        Color32::from_rgb(0xEF, 0xF4, 0xFF)
    }
}

/// Soft purple for thought blocks.
pub fn purple_soft() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(167, 139, 250, 22)
    } else {
        Color32::from_rgb(0xF5, 0xF3, 0xFF)
    }
}

/// Assistant bubble fill — must lift clearly off stage (esp. dark mode).
pub fn assistant_panel() -> Color32 {
    if is_dark() {
        // #1C222C on #0F1115 — clear but not harsh
        Color32::from_rgb(0x1C, 0x22, 0x2C)
    } else {
        Color32::WHITE
    }
}

pub fn assistant_panel_border() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x34, 0x3E, 0x4F)
    } else {
        Color32::from_rgb(0xE2, 0xE5, 0xEB)
    }
}

/// Grok avatar plate (chat row).
pub fn grok_avatar_plate() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x2A, 0x31, 0x3D)
    } else {
        Color32::from_rgb(0xEE, 0xF0, 0xF5)
    }
}

pub fn grok_avatar_ring() -> Color32 {
    if is_dark() {
        Color32::from_rgb(0x4B, 0x55, 0x68)
    } else {
        Color32::from_rgb(0xD0, 0xD4, 0xDC)
    }
}

pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

/// Connection status dot color.
pub fn status_color(connected: bool, connecting: bool) -> Color32 {
    if connecting {
        t().warning
    } else if connected {
        t().success
    } else {
        t().text_3
    }
}

/// Apply palette visuals. Font scale is handled separately by `fonts::install` + app.
pub fn apply(ctx: &egui::Context, dark: bool) {
    set_dark(dark);
    let p = t();
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.override_text_color = Some(p.text);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_2);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text_2);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    visuals.selection.bg_fill = if dark {
        Color32::from_rgba_unmultiplied(59, 130, 246, 55)
    } else {
        Color32::from_rgba_unmultiplied(37, 99, 235, 40)
    };
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
    visuals.extreme_bg_color = p.code_bg;
    visuals.code_bg_color = p.code_bg;
    visuals.faint_bg_color = p.surface_2;
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.inactive.bg_fill = p.surface_2;
    visuals.widgets.hovered.bg_fill = p.hover;
    visuals.widgets.active.bg_fill = p.selected;
    visuals.widgets.open.bg_fill = p.selected;
    visuals.hyperlink_color = p.accent;
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.danger;
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(8);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    // Body stays readable; headings for assistant prose
    if let Some(body) = style.text_styles.get_mut(&TextStyle::Body) {
        body.size = body.size.max(14.0);
    }
    if let Some(heading) = style.text_styles.get_mut(&TextStyle::Heading) {
        heading.size = heading.size.max(17.0);
    }
    ctx.set_style(style);
    let _ = (FontFamily::Proportional, FontId::default(), Margin::ZERO);
}
