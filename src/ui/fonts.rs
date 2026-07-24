//! Load Windows system fonts so Chinese renders in egui.
//! Default egui fonts have almost no CJK glyphs.
//!
//! IMPORTANT: Do NOT load Segoe UI Emoji / color fonts — ab_glyph can panic
//! on COLR/CBDT fonts and take the whole process down (flash-exit on Windows).

use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// Install proportional + monospace fonts with CJK coverage.
/// Never panics — on failure keeps egui defaults.
pub fn install(ctx: &egui::Context) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_inner(ctx)));
    if let Err(e) = result {
        warn!("font install panicked (kept default fonts): {e:?}");
    }
}

fn install_inner(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let cjk_name = load_cjk_into(&mut fonts);

    // Latin UI — plain TTF only
    if let Some(data) = read_plain_ttf(&[font_path("segoeui.ttf"), font_path("arial.ttf")]) {
        fonts.font_data.insert("segoe".into(), Arc::new(data));
        info!("UI font (Latin): segoeui/arial");
    }

    // Monospace
    if let Some(data) = read_plain_ttf(&[
        font_path("consola.ttf"),
        font_path("CascadiaMono.ttf"),
        font_path("cour.ttf"),
    ]) {
        fonts.font_data.insert("consolas".into(), Arc::new(data));
    }

    // NOTE: deliberately skip emoji fonts (seguiemj.ttf etc.) — they crash ab_glyph.

    let mut prop: Vec<String> = Vec::new();
    if fonts.font_data.contains_key("segoe") {
        prop.push("segoe".into());
    }
    if let Some(ref name) = cjk_name {
        prop.push(name.clone());
    }
    append_defaults(&fonts, FontFamily::Proportional, &mut prop);
    fonts.families.insert(FontFamily::Proportional, prop);

    let mut mono: Vec<String> = Vec::new();
    if fonts.font_data.contains_key("consolas") {
        mono.push("consolas".into());
    }
    if let Some(ref name) = cjk_name {
        mono.push(name.clone());
    }
    append_defaults(&fonts, FontFamily::Monospace, &mut mono);
    fonts.families.insert(FontFamily::Monospace, mono);

    if cjk_name.is_none() {
        warn!("未找到中文字体（simhei）。中文可能显示为空白。");
    }

    ctx.set_fonts(fonts);
    info!("fonts installed ok");
}

fn load_cjk_into(fonts: &mut FontDefinitions) -> Option<String> {
    // Prefer pure TTF. Avoid .ttc collections when possible (face index issues).
    let candidates: &[(&str, &str)] = &[
        ("simhei", "simhei.ttf"),
        ("msyh", "msyh.ttf"),
        // msyh.ttc only as last resort — index 0
    ];

    for (name, file) in candidates {
        let path = font_path(file);
        if let Some(data) = read_plain_ttf(&[path.clone()]) {
            info!("UI font (CJK): {} ({})", name, path.display());
            fonts.font_data.insert((*name).to_string(), Arc::new(data));
            return Some((*name).to_string());
        }
    }

    // Last resort: 微软雅黑 collection
    let ttc = font_path("msyh.ttc");
    if let Some(data) = read_font_indexed(&[ttc.clone()], 0) {
        info!("UI font (CJK): yahei ttc ({})", ttc.display());
        fonts.font_data.insert("yahei".into(), Arc::new(data));
        return Some("yahei".into());
    }
    None
}

fn read_plain_ttf(paths: &[PathBuf]) -> Option<FontData> {
    read_font_indexed(paths, 0)
}

fn read_font_indexed(paths: &[PathBuf], index: u32) -> Option<FontData> {
    for path in paths {
        if !path.is_file() {
            continue;
        }
        // Skip known-bad / huge color fonts
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name.contains("emoji") || name.contains("seguiemj") || name.contains("color") {
            warn!("skip emoji/color font {}", path.display());
            continue;
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                // Sanity: reject empty / tiny
                if bytes.len() < 1000 {
                    continue;
                }
                return Some(FontData {
                    font: std::borrow::Cow::Owned(bytes),
                    index,
                    tweak: FontTweak::default(),
                });
            }
            Err(e) => warn!("read font {}: {e}", path.display()),
        }
    }
    None
}

fn font_path(name: &str) -> PathBuf {
    if let Ok(windir) = std::env::var("WINDIR") {
        return Path::new(&windir).join("Fonts").join(name);
    }
    PathBuf::from(r"C:\Windows\Fonts").join(name)
}

fn append_defaults(fonts: &FontDefinitions, family: FontFamily, out: &mut Vec<String>) {
    if let Some(defaults) = fonts.families.get(&family) {
        for d in defaults {
            if !out.iter().any(|p| p == d) {
                out.push(d.clone());
            }
        }
    }
}
