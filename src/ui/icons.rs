//! Brand mark + simple vector icons (no emoji — Windows often shows empty boxes).
//! Grok logo is the official mark from RongleCat/grok-app (`assets/logo.png`).

use super::theme;
use egui::{
    Color32, ColorImage, Id, Pos2, Rect, Response, Sense, Shape, Stroke, TextureHandle, Ui, Vec2,
};

const LOGO_PNG: &[u8] = include_bytes!("../../assets/logo.png");

/// Load mark as **white-on-transparent** so `.tint()` works in light & dark.
/// Source PNG has a solid black plate — we strip it and keep luminance as alpha.
fn logo_color_image() -> ColorImage {
    let img = image::load_from_memory(LOGO_PNG)
        .unwrap_or_else(|_| image::DynamicImage::new_rgba8(32, 32));
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let mut pixels = Vec::with_capacity(w * h * 4);
    for px in rgba.pixels() {
        let [r, g, b, a] = px.0;
        // Luminance of the silver mark; pure black plate → transparent
        let lum = ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) / 256) as u8;
        let src_a = a;
        // Soft threshold: keep soft anti-aliased edges of the glyph
        let mark_a = if lum < 28 {
            0u8
        } else {
            // Map lum 28..255 → alpha, multiply by source alpha
            let t = ((lum as u16 - 28) * 255 / (255 - 28)) as u8;
            ((t as u16 * src_a as u16) / 255) as u8
        };
        // Pure white RGB; color comes entirely from egui tint
        pixels.extend_from_slice(&[255, 255, 255, mark_a]);
    }
    ColorImage::from_rgba_unmultiplied([w, h], &pixels)
}

fn logo_texture(ctx: &egui::Context) -> TextureHandle {
    // Bump id when decode logic changes so stale black-plate textures are dropped.
    let id = Id::new("grok_brand_logo_tex_v2");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return tex;
    }
    let color = logo_color_image();
    let tex = ctx.load_texture("grok_brand_logo_v2", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// Tint: near-white on dark chrome, near-black on light chrome (like SVG currentColor).
fn logo_tint() -> Color32 {
    if theme::is_dark() {
        Color32::from_rgb(0xF2, 0xF2, 0xF2)
    } else {
        // Strong contrast on light sidebar (#ececef)
        Color32::from_rgb(0x11, 0x11, 0x14)
    }
}

/// Paint logo into an existing rect (no layout allocation).
pub fn paint_logo(ui: &mut Ui, rect: Rect) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let tex = logo_texture(ui.ctx());
    let pad = rect.width() * 0.04;
    let r = rect.shrink(pad);
    egui::Image::new((tex.id(), r.size()))
        .tint(logo_tint())
        .paint_at(ui, r);
}

/// Grok brand mark (sidebar header, empty chat, assistant avatar).
pub fn grok_logo(ui: &mut Ui, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_logo(ui, rect);
    resp
}

/// Assistant avatar: full Grok logo (no circular plate).
pub fn grok_avatar(ui: &mut Ui, size: f32) {
    let _ = grok_logo(ui, size);
}

/// User avatar: custom image (circular) or letter badge.
pub fn user_avatar(ui: &mut Ui, size: f32, letter: &str) {
    user_avatar_ex(ui, size, letter, None);
}

/// User avatar with optional local image path (png/jpg/…); falls back to letter.
pub fn user_avatar_ex(ui: &mut Ui, size: f32, letter: &str, image_path: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let p = theme::t();
    let r = size * 0.5;
    let painter = ui.painter();
    // Soft drop
    if theme::is_dark() {
        painter.circle_filled(
            rect.center() + Vec2::new(0.0, 1.0),
            r,
            Color32::from_black_alpha(50),
        );
    }

    if let Some(path) = image_path.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(tex) = load_user_avatar_texture(ui.ctx(), path) {
            egui::Image::new((tex.id(), rect.size()))
                .corner_radius(size * 0.5)
                .paint_at(ui, rect);
            let ring = if theme::is_dark() {
                Color32::from_rgb(0x60, 0xA5, 0xFA)
            } else {
                Color32::from_rgb(0x93, 0xC5, 0xFD)
            };
            painter.circle_stroke(rect.center(), r - 0.5, Stroke::new(1.5, ring));
            return;
        }
    }

    // Letter badge: solid accent + white glyph + light ring
    painter.circle_filled(rect.center(), r, p.accent);
    painter.circle_stroke(
        rect.center(),
        r - 0.5,
        Stroke::new(
            1.25,
            if theme::is_dark() {
                Color32::from_rgb(0x93, 0xC5, 0xFD)
            } else {
                Color32::from_rgb(0xBF, 0xDB, 0xFE)
            },
        ),
    );
    let glyph = if letter.is_empty() {
        crate::i18n::t().me
    } else {
        letter
    };
    let ch: String = glyph.chars().take(1).collect();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        ch,
        egui::FontId::proportional((size * 0.44).clamp(11.0, 16.0)),
        p.on_accent,
    );
}

fn load_user_avatar_texture(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    use std::path::Path;
    let p = Path::new(path);
    if !p.is_file() {
        return None;
    }
    // Cache key includes path + mtime so edits refresh
    let mtime = std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = Id::new(("user_avatar_tex", path, mtime));
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let bytes = std::fs::read(p).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    // Center-crop to square for circular avatars
    let side = w.min(h);
    let x0 = (w.saturating_sub(side)) / 2;
    let y0 = (h.saturating_sub(side)) / 2;
    let mut square = image::RgbaImage::new(side as u32, side as u32);
    for yy in 0..side {
        for xx in 0..side {
            square.put_pixel(
                xx as u32,
                yy as u32,
                *img.get_pixel((x0 + xx) as u32, (y0 + yy) as u32),
            );
        }
    }
    // Downscale if huge (keep UI snappy)
    let rgba = if side > 256 {
        image::DynamicImage::ImageRgba8(square)
            .resize_exact(256, 256, image::imageops::FilterType::Triangle)
            .to_rgba8()
    } else {
        square
    };
    let (tw, th) = (rgba.width() as usize, rgba.height() as usize);
    let color = ColorImage::from_rgba_unmultiplied([tw, th], rgba.as_raw());
    let tex = ctx.load_texture(
        format!("user_avatar_{mtime}"),
        color,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

// ─── Stroke icons (Tabler-inspired, 16×16 optical box) ───────────────────────

/// Stroke width scales with glyph box (~1.75 @ 16px, Tabler default).
fn stroke_for(rect: Rect, color: Color32) -> Stroke {
    let w = (rect.width() / 16.0 * 1.75).clamp(1.2, 2.1);
    Stroke::new(w, color)
}

fn paint_icon_bg(ui: &mut Ui, rect: Rect, hovered: bool) {
    if hovered {
        ui.painter().rect_filled(rect, 6.0, theme::HOVER());
    }
}

/// Round-cap line (egui strokes are butt-ended; caps via end dots).
fn line_cap(painter: &egui::Painter, a: Pos2, b: Pos2, s: Stroke) {
    painter.line_segment([a, b], s);
    let r = s.width * 0.5;
    painter.circle_filled(a, r, s.color);
    painter.circle_filled(b, r, s.color);
}

fn poly_cap(painter: &egui::Painter, pts: &[Pos2], s: Stroke) {
    if pts.len() < 2 {
        return;
    }
    painter.add(Shape::line(pts.to_vec(), s));
    let r = s.width * 0.5;
    if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
        painter.circle_filled(*first, r, s.color);
        painter.circle_filled(*last, r, s.color);
    }
}

fn arc_pts(center: Pos2, rad: f32, start_deg: f32, end_deg: f32, steps: usize) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = (start_deg + (end_deg - start_deg) * t).to_radians();
        pts.push(Pos2::new(
            center.x + a.cos() * rad,
            center.y + a.sin() * rad,
        ));
    }
    pts
}

/// Icon button: soft hover, optically centered glyph.
pub fn icon_btn(ui: &mut Ui, kind: IconKind, tip: &str) -> Response {
    let size = theme::ICON_BTN;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        paint_icon_bg(ui, rect, resp.hovered());
        let color = if resp.hovered() {
            theme::TEXT()
        } else {
            theme::TEXT_2()
        };
        // 16px optical box inside 28px hit target (~57% fill)
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.58));
        draw_kind(ui, kind, glyph, color);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

#[derive(Clone, Copy)]
pub enum IconKind {
    Plus,
    Settings,
    Share,
    Sidebar,
    SidebarOpen,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronRightSmall,
    Folder,
    Refresh,
    Paperclip,
    Send,
    Logs,
    Sun,
    Moon,
    Search,
    Close,
    Check,
    Dot,
}

fn draw_kind(ui: &mut Ui, kind: IconKind, rect: Rect, color: Color32) {
    let painter = ui.painter();
    let s = stroke_for(rect, color);
    let c = rect.center();
    let w = rect.width();
    let h = rect.height();
    // Normalized helpers (0..1 inside rect)
    let nx = |t: f32| rect.left() + w * t;
    let ny = |t: f32| rect.top() + h * t;

    match kind {
        IconKind::Plus => {
            line_cap(
                painter,
                Pos2::new(c.x, ny(0.18)),
                Pos2::new(c.x, ny(0.82)),
                s,
            );
            line_cap(
                painter,
                Pos2::new(nx(0.18), c.y),
                Pos2::new(nx(0.82), c.y),
                s,
            );
        }

        IconKind::Settings => {
            // Clean gear: ring + 6 short teeth (not 8 noisy radials)
            let r_in = w * 0.22;
            let r_out = w * 0.38;
            let r_mid = w * 0.30;
            painter.circle_stroke(c, r_mid, s);
            painter.circle_stroke(c, r_in * 0.55, s);
            for i in 0..6 {
                let a = (i as f32 * 60.0 - 90.0).to_radians();
                let a2 = a + 12f32.to_radians();
                let a0 = a - 12f32.to_radians();
                // tooth as short thick radial
                let inner = Pos2::new(c.x + a.cos() * r_mid, c.y + a.sin() * r_mid);
                let outer = Pos2::new(c.x + a.cos() * r_out, c.y + a.sin() * r_out);
                line_cap(painter, inner, outer, s);
                let _ = (a0, a2);
            }
        }

        IconKind::Share => {
            // Three connected nodes — familiar share glyph with the same
            // rounded stroke language as the rest of the sidebar icons.
            let left = Pos2::new(nx(0.27), c.y);
            let top = Pos2::new(nx(0.71), ny(0.25));
            let bottom = Pos2::new(nx(0.71), ny(0.75));
            line_cap(painter, left, top, s);
            line_cap(painter, left, bottom, s);
            let node_r = w * 0.105;
            for node in [left, top, bottom] {
                painter.circle_filled(node, node_r + s.width * 0.35, color);
            }
        }

        IconKind::Sidebar | IconKind::SidebarOpen => {
            let body =
                Rect::from_min_max(Pos2::new(nx(0.12), ny(0.14)), Pos2::new(nx(0.88), ny(0.86)));
            painter.rect_stroke(body, 2.2, s, egui::StrokeKind::Middle);
            let x = body.left() + body.width() * 0.34;
            line_cap(
                painter,
                Pos2::new(x, body.top() + 1.0),
                Pos2::new(x, body.bottom() - 1.0),
                s,
            );
        }

        IconKind::ChevronLeft => {
            poly_cap(
                painter,
                &[
                    Pos2::new(nx(0.58), ny(0.22)),
                    Pos2::new(nx(0.36), c.y),
                    Pos2::new(nx(0.58), ny(0.78)),
                ],
                s,
            );
        }
        IconKind::ChevronRight | IconKind::ChevronRightSmall => {
            poly_cap(
                painter,
                &[
                    Pos2::new(nx(0.42), ny(0.22)),
                    Pos2::new(nx(0.64), c.y),
                    Pos2::new(nx(0.42), ny(0.78)),
                ],
                s,
            );
        }
        IconKind::ChevronDown => {
            poly_cap(
                painter,
                &[
                    Pos2::new(nx(0.22), ny(0.40)),
                    Pos2::new(c.x, ny(0.64)),
                    Pos2::new(nx(0.78), ny(0.40)),
                ],
                s,
            );
        }

        IconKind::Folder => {
            // Tabler-style folder outline
            let tab_bl = Pos2::new(nx(0.12), ny(0.38));
            let tab_tl = Pos2::new(nx(0.12), ny(0.30));
            let tab_tr = Pos2::new(nx(0.42), ny(0.30));
            let tab_br = Pos2::new(nx(0.50), ny(0.38));
            let body_br = Pos2::new(nx(0.88), ny(0.38));
            let body_tr = Pos2::new(nx(0.88), ny(0.78));
            let body_bl = Pos2::new(nx(0.12), ny(0.78));
            poly_cap(
                painter,
                &[
                    tab_bl, tab_tl, tab_tr, tab_br, body_br, body_tr, body_bl, tab_bl,
                ],
                s,
            );
        }

        IconKind::Refresh => {
            // Classic circular arrow (Tabler `refresh`): ~280° arc + chevron head
            let rad = w * 0.34;
            let start = 50.0_f32;
            let end = 330.0_f32;
            let pts = arc_pts(c, rad, start, end, 24);
            poly_cap(painter, &pts, s);
            if let Some(&tip) = pts.last() {
                // Tangent at end angle (CCW arc: tangent is perpendicular)
                let a = end.to_radians();
                let tx = -a.sin(); // direction of motion
                let ty = a.cos();
                let px = -ty; // perpendicular
                let py = tx;
                let ah = w * 0.20;
                let base = Pos2::new(tip.x - tx * ah * 0.85, tip.y - ty * ah * 0.85);
                let left = Pos2::new(base.x + px * ah * 0.55, base.y + py * ah * 0.55);
                let right = Pos2::new(base.x - px * ah * 0.55, base.y - py * ah * 0.55);
                line_cap(painter, tip, left, s);
                line_cap(painter, tip, right, s);
            }
        }

        IconKind::Paperclip => {
            // Vertical paperclip (simple, readable at 16px)
            let clip = [
                Pos2::new(nx(0.58), ny(0.78)),
                Pos2::new(nx(0.58), ny(0.28)),
                Pos2::new(nx(0.42), ny(0.16)),
                Pos2::new(nx(0.30), ny(0.28)),
                Pos2::new(nx(0.30), ny(0.72)),
                Pos2::new(nx(0.40), ny(0.84)),
                Pos2::new(nx(0.52), ny(0.72)),
                Pos2::new(nx(0.52), ny(0.36)),
                Pos2::new(nx(0.42), ny(0.28)),
                Pos2::new(nx(0.38), ny(0.36)),
                Pos2::new(nx(0.38), ny(0.64)),
            ];
            poly_cap(painter, &clip, s);
        }

        IconKind::Send => {
            // Filled paper-plane pointing up-right
            let tip = Pos2::new(nx(0.86), ny(0.22));
            let bl = Pos2::new(nx(0.14), ny(0.38));
            let mid = Pos2::new(nx(0.42), ny(0.50));
            let br = Pos2::new(nx(0.30), ny(0.82));
            painter.add(Shape::convex_polygon(
                vec![tip, bl, mid],
                color,
                Stroke::NONE,
            ));
            painter.add(Shape::convex_polygon(
                vec![tip, mid, br],
                color.gamma_multiply(0.85),
                Stroke::NONE,
            ));
            // fold line
            line_cap(
                painter,
                mid,
                tip,
                Stroke::new(s.width * 0.7, color.gamma_multiply(0.5)),
            );
        }

        IconKind::Logs => {
            // List with leading dots (not three equal bars)
            for i in 0..3 {
                let y = ny(0.28 + i as f32 * 0.22);
                painter.circle_filled(Pos2::new(nx(0.20), y), s.width * 0.55, color);
                line_cap(painter, Pos2::new(nx(0.34), y), Pos2::new(nx(0.82), y), s);
            }
        }

        IconKind::Sun => {
            painter.circle_stroke(c, w * 0.18, s);
            for i in 0..8 {
                let a = (i as f32 * 45.0 - 90.0).to_radians();
                let i0 = Pos2::new(c.x + a.cos() * w * 0.28, c.y + a.sin() * w * 0.28);
                let i1 = Pos2::new(c.x + a.cos() * w * 0.40, c.y + a.sin() * w * 0.40);
                line_cap(painter, i0, i1, s);
            }
        }

        IconKind::Moon => {
            // Crescent: long outer arc only
            let r = w * 0.34;
            let outer = arc_pts(c, r, 55.0, 305.0, 18);
            poly_cap(painter, &outer, s);
            // Inner arc for hollow crescent
            let inner_c = Pos2::new(c.x + w * 0.12, c.y - h * 0.04);
            let inner = arc_pts(inner_c, r * 0.72, 80.0, 280.0, 14);
            poly_cap(painter, &inner, s);
        }

        IconKind::Search => {
            let cc = Pos2::new(nx(0.42), ny(0.42));
            painter.circle_stroke(cc, w * 0.26, s);
            line_cap(
                painter,
                Pos2::new(nx(0.60), ny(0.60)),
                Pos2::new(nx(0.82), ny(0.82)),
                s,
            );
        }

        IconKind::Close => {
            line_cap(
                painter,
                Pos2::new(nx(0.24), ny(0.24)),
                Pos2::new(nx(0.76), ny(0.76)),
                s,
            );
            line_cap(
                painter,
                Pos2::new(nx(0.76), ny(0.24)),
                Pos2::new(nx(0.24), ny(0.76)),
                s,
            );
        }

        IconKind::Check => {
            poly_cap(
                painter,
                &[
                    Pos2::new(nx(0.18), ny(0.52)),
                    Pos2::new(nx(0.40), ny(0.74)),
                    Pos2::new(nx(0.82), ny(0.28)),
                ],
                Stroke::new(s.width * 1.15, color),
            );
        }

        IconKind::Dot => {
            painter.circle_filled(c, w * 0.16, color);
        }
    }
}

/// Paint icon inside an already-allocated rect (for custom rows).
pub fn paint_in(ui: &mut Ui, kind: IconKind, rect: Rect, color: Color32) {
    // Keep a little inset so strokes aren't clipped
    let r = if rect.width() > 10.0 {
        rect.shrink(0.5)
    } else {
        rect
    };
    draw_kind(ui, kind, r, color);
}

/// Small folder icon for project rows.
pub fn folder_glyph(ui: &mut Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        draw_kind(ui, IconKind::Folder, rect.shrink(0.5), color);
    }
}

/// Small chevron for collapse state.
pub fn chevron_glyph(ui: &mut Ui, size: f32, collapsed: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let k = if collapsed {
            IconKind::ChevronRightSmall
        } else {
            IconKind::ChevronDown
        };
        draw_kind(ui, k, rect.shrink(1.0), color);
    }
}
