//! Pending image/file attachments for the composer (clipboard paste + file pick).
//!
//! Images are **saved under the app data dir** and sent to the agent as path refs
//! (`图1: @C:\...\1.png`) — same pattern as RongleCat/grok-app `@path` attachments.

use anyhow::{bail, Context, Result};
use base64::Engine;
use image::ImageEncoder;
use std::path::{Path, PathBuf};
use tracing::debug;

const MAX_SIDE: u32 = 2048;
const MAX_BYTES: usize = 12 * 1024 * 1024; // 12 MB raw
/// Max attachments per message (product limit).
pub const MAX_ATTACHMENTS: usize = 9;

#[derive(Clone)]
pub struct PendingImage {
    pub id: String,
    pub name: String,
    pub mime: String,
    /// PNG bytes (local disk + UI decode source).
    pub png_bytes: Vec<u8>,
    /// RGBA preview for thumbnails.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Absolute path on disk under app attachments dir (preferred for agent).
    pub disk_path: Option<PathBuf>,
    /// 1-based label index shown as 图N (set when queued).
    pub label_index: u32,
}

impl PendingImage {
    pub fn summary_label(&self) -> String {
        if self.label_index > 0 {
            format!(
                "图{} · {} ({}×{})",
                self.label_index, self.name, self.width, self.height
            )
        } else {
            format!("[图] {} ({}×{})", self.name, self.width, self.height)
        }
    }

    /// Persist PNG under `%config%/GrokApp/attachments/YYYYMMDD/…`.
    pub fn ensure_on_disk(&mut self, index: u32) -> Result<PathBuf> {
        self.label_index = index;
        if let Some(p) = &self.disk_path {
            if p.is_file() {
                return Ok(p.clone());
            }
        }
        let dir = attachments_dir()?;
        std::fs::create_dir_all(&dir)?;
        let safe = sanitize_filename(&self.name);
        let fname = format!("{:02}_{}_{}.png", index, &self.id[..8.min(self.id.len())], safe);
        let path = dir.join(fname);
        std::fs::write(&path, &self.png_bytes)
            .with_context(|| format!("write attachment {}", path.display()))?;
        self.disk_path = Some(path.clone());
        self.name = format!("图{index}.png");
        Ok(path)
    }

    /// Convert to a chat-timeline image (optionally downscale for UI memory).
    pub fn to_chat_image(&self) -> crate::acp::ChatImage {
        const MAX_UI: u32 = 1024;
        let (w, h, rgba) = if self.width > MAX_UI || self.height > MAX_UI {
            let img = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
                .unwrap_or_else(|| image::RgbaImage::new(1, 1));
            let nw = if self.width >= self.height {
                MAX_UI
            } else {
                ((self.width as f32 / self.height as f32) * MAX_UI as f32) as u32
            }
            .max(1);
            let nh = if self.height >= self.width {
                MAX_UI
            } else {
                ((self.height as f32 / self.width as f32) * MAX_UI as f32) as u32
            }
            .max(1);
            let resized =
                image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
            let (rw, rh) = resized.dimensions();
            (rw, rh, resized.into_raw())
        } else {
            (self.width, self.height, self.rgba.clone())
        };
        crate::acp::ChatImage {
            id: self.id.clone(),
            label: self.summary_label(),
            width: w,
            height: h,
            rgba,
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("img");
    let s: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(24)
        .collect();
    if s.is_empty() {
        "img".into()
    } else {
        s
    }
}

pub fn attachments_dir() -> Result<PathBuf> {
    let base = crate::config::AppConfig::config_dir()?;
    let day = chrono::Local::now().format("%Y%m%d").to_string();
    Ok(base.join("attachments").join(day))
}

/// Try to read an image from the system clipboard.
pub fn from_clipboard() -> Option<PendingImage> {
    match from_clipboard_ex() {
        Ok(img) => img,
        Err(e) => {
            debug!("clipboard image: {e}");
            None
        }
    }
}

/// Cheap check: does the clipboard advertise an image-like format?
/// Does **not** open the clipboard (IsClipboardFormatAvailable is lock-free).
pub fn clipboard_has_image_hint() -> bool {
    #[cfg(windows)]
    {
        return windows_clipboard_has_image_format();
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Result of a clipboard probe for diagnostics.
#[derive(Debug, Clone)]
pub struct ClipboardProbe {
    pub has_image: bool,
    pub formats: String,
    pub detail: String,
}

pub fn probe_clipboard() -> ClipboardProbe {
    #[cfg(windows)]
    {
        return windows_probe();
    }
    #[cfg(not(windows))]
    {
        ClipboardProbe {
            has_image: false,
            formats: String::new(),
            detail: "non-windows".into(),
        }
    }
}

/// Same as [`from_clipboard`] with error detail.
pub fn from_clipboard_ex() -> Result<Option<PendingImage>, String> {
    #[cfg(windows)]
    {
        // Multiple attempts: OpenClipboard is exclusive; egui-winit/arboard may
        // hold it briefly on Ctrl+V.
        let mut last_err = String::new();
        for attempt in 0..8 {
            match windows_clipboard_image() {
                Ok(Some(img)) => return Ok(Some(img)),
                Ok(None) => {
                    // Successfully opened, nothing image-like — stop early.
                    return Ok(None);
                }
                Err(e) => {
                    last_err = e.to_string();
                    debug!("windows clipboard attempt {attempt}: {e:#}");
                    std::thread::sleep(std::time::Duration::from_millis(12 + attempt * 4));
                }
            }
        }
        Err(last_err)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

/// Decode image from a paste string: data URI, file path, file://, HTML img src.
pub fn from_paste_payload(text: &str) -> Option<PendingImage> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }

    if let Some(rest) = t.strip_prefix("data:image/") {
        if let Some((meta, b64)) = rest.split_once(";base64,") {
            let mime_sub = meta.split(';').next().unwrap_or("png");
            let name = format!("paste.{mime_sub}");
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                return from_bytes(&bytes, &name).ok();
            }
            if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(b64.trim()) {
                return from_bytes(&bytes, &name).ok();
            }
        }
    }

    let path_str = if let Some(rest) = t.strip_prefix("file:///") {
        rest.to_string()
    } else if let Some(rest) = t.strip_prefix("file://") {
        rest.to_string()
    } else {
        t.to_string()
    };
    let path_str = path_str.trim_matches('"').trim_matches('\'');
    let path = Path::new(path_str);
    if path.is_file() {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff"
        ) {
            return from_path(path).ok();
        }
    }

    if t.contains("<img") || t.contains("<IMG") {
        if let Some(src) = extract_img_src(t) {
            return from_paste_payload(&src);
        }
    }

    None
}

fn extract_img_src(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let idx = lower.find("src=")?;
    let after = html[idx + 4..].trim_start();
    let quote = after.chars().next()?;
    if quote == '"' || quote == '\'' {
        let rest = &after[1..];
        let end = rest.find(quote)?;
        return Some(rest[..end].to_string());
    }
    let end = after
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(after.len());
    Some(after[..end].to_string())
}

pub fn from_path(path: &Path) -> Result<PendingImage> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.len() > MAX_BYTES {
        bail!("文件过大（>12MB）");
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();
    from_bytes(&bytes, &name)
}

pub fn from_bytes(bytes: &[u8], name: &str) -> Result<PendingImage> {
    if bytes.len() > MAX_BYTES {
        bail!("文件过大（>12MB）");
    }
    let dyn_img = image::load_from_memory(bytes).context("解码图片失败")?;
    let rgba_img = dyn_img.to_rgba8();
    let (w, h) = rgba_img.dimensions();
    from_rgba(rgba_img.into_raw(), w, h, name)
}

fn from_rgba(rgba: Vec<u8>, w: u32, h: u32, name: &str) -> Result<PendingImage> {
    let (w, h, rgba) = if w > MAX_SIDE || h > MAX_SIDE {
        let img = image::RgbaImage::from_raw(w, h, rgba).context("invalid rgba buffer")?;
        let resized = image::imageops::resize(
            &img,
            if w >= h {
                MAX_SIDE
            } else {
                ((w as f32 / h as f32) * MAX_SIDE as f32) as u32
            }
            .max(1),
            if h >= w {
                MAX_SIDE
            } else {
                ((h as f32 / w as f32) * MAX_SIDE as f32) as u32
            }
            .max(1),
            image::imageops::FilterType::Triangle,
        );
        let (nw, nh) = resized.dimensions();
        debug!("resized attachment {w}x{h} -> {nw}x{nh}");
        (nw, nh, resized.into_raw())
    } else {
        (w, h, rgba)
    };

    let mut png_bytes = Vec::new();
    {
        let enc = image::codecs::png::PngEncoder::new(&mut png_bytes);
        enc.write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
            .context("encode png")?;
    }

    Ok(PendingImage {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        mime: "image/png".into(),
        png_bytes,
        rgba,
        width: w,
        height: h,
        disk_path: None,
        label_index: 0,
    })
}

/// Build ACP multimodal prompt blocks: text + **image** content (base64).
///
/// Prefer native image blocks so the model can see the picture without calling
/// `read_file` / `fs/read_text_file` on a PNG (which used to hang the turn when
/// host fs replies were dropped). Path notes are still included for reference.
pub fn build_prompt_blocks(text: &str, images: &[PendingImage]) -> Vec<serde_json::Value> {
    let mut blocks: Vec<serde_json::Value> = Vec::new();
    let t = text.trim();
    if !t.is_empty() {
        blocks.push(serde_json::json!({ "type": "text", "text": t }));
    } else if !images.is_empty() {
        blocks.push(serde_json::json!({
            "type": "text",
            "text": "请直接查看以下图片并回答。"
        }));
    }
    for (i, img) in images.iter().enumerate() {
        let n = if img.label_index > 0 {
            img.label_index
        } else {
            (i + 1) as u32
        };
        let path = img
            .disk_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| img.name.clone());
        // Small caption so the model knows the label; pixels follow as image block.
        blocks.push(serde_json::json!({
            "type": "text",
            "text": format!("图{n}: {path} ({}×{})", img.width, img.height)
        }));
        let b64 = base64::engine::general_purpose::STANDARD.encode(&img.png_bytes);
        // Cap very large payloads (~keep under ~4MB base64 of PNG)
        if b64.len() > 5_500_000 {
            blocks.push(serde_json::json!({
                "type": "text",
                "text": format!("[图{n} 过大已省略内嵌像素，路径: {path}]")
            }));
            continue;
        }
        let mut image_block = serde_json::json!({
            "type": "image",
            "mimeType": if img.mime.is_empty() { "image/png".into() } else { img.mime.clone() },
            "data": b64,
        });
        if let Some(path) = &img.disk_path {
            image_block["uri"] = serde_json::json!(format!(
                "file:///{}",
                path.display().to_string().replace('\\', "/")
            ));
        }
        blocks.push(image_block);
    }
    blocks
}

// ---------------------------------------------------------------------------
// Windows clipboard
//
// IMPORTANT: Open clipboard ONCE. Do NOT call get_clipboard() while holding
// Clipboard — that tries OpenClipboard again and fails on Windows.
// Use format.read_clipboard() while the guard is alive.
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn windows_clipboard_has_image_format() -> bool {
    use clipboard_win::{formats, is_format_avail, register_format};
    // IsClipboardFormatAvailable does not require OpenClipboard.
    if is_format_avail(formats::CF_BITMAP)
        || is_format_avail(formats::CF_DIB)
        || is_format_avail(formats::CF_DIBV5)
        || is_format_avail(formats::CF_HDROP)
    {
        return true;
    }
    for name in ["PNG", "JFIF", "JPEG", "GIF", "image/png", "image/jpeg"] {
        if let Some(fmt) = register_format(name) {
            if is_format_avail(fmt.get()) {
                return true;
            }
        }
    }
    false
}

#[cfg(windows)]
fn windows_probe() -> ClipboardProbe {
    use clipboard_win::{formats, is_format_avail, register_format};
    let mut parts = Vec::new();
    if is_format_avail(formats::CF_BITMAP) {
        parts.push("BITMAP");
    }
    if is_format_avail(formats::CF_DIB) {
        parts.push("DIB");
    }
    if is_format_avail(formats::CF_DIBV5) {
        parts.push("DIBV5");
    }
    if is_format_avail(formats::CF_HDROP) {
        parts.push("HDROP");
    }
    for name in ["PNG", "JFIF", "JPEG"] {
        if let Some(f) = register_format(name) {
            if is_format_avail(f.get()) {
                parts.push(name);
            }
        }
    }
    let formats = parts.join(",");
    ClipboardProbe {
        has_image: !parts.is_empty(),
        formats: formats.clone(),
        detail: if parts.is_empty() {
            "no image formats advertised".into()
        } else {
            format!("formats={formats}")
        },
    }
}

#[cfg(windows)]
fn windows_clipboard_image() -> Result<Option<PendingImage>> {
    // Getter is a crate-root trait (not re-exported under formats).
    use clipboard_win::formats::{self, Bitmap, FileList, Unicode};
    use clipboard_win::{is_format_avail, raw, register_format, Clipboard, Getter};

    // Single open for the whole read sequence.
    // Never call get_clipboard() while holding this — it re-opens and fails.
    let _clip = Clipboard::new_attempts(20).map_err(|e| anyhow::anyhow!("OpenClipboard: {e}"))?;

    // NOTE: raw::get(&mut [u8]) needs a pre-sized buffer. Empty Vec → always 0 bytes.
    // Always use raw::get_vec for growable reads.

    // 1) PNG — Win+Shift+S (Win10 1809+), Chrome, Edge, Firefox
    for name in ["PNG", "image/png"] {
        if let Some(fmt) = register_format(name) {
            if is_format_avail(fmt.get()) {
                let mut buf = Vec::new();
                match raw::get_vec(fmt.get(), &mut buf) {
                    Ok(_) if buf.len() > 24 => {
                        // Accept even if magic is odd (some sources prefix junk).
                        if let Ok(img) = from_bytes(&buf, "paste.png") {
                            return Ok(Some(img));
                        }
                        if let Some(i) = buf.windows(8).position(|w| w == b"\x89PNG\r\n\x1a\n") {
                            if let Ok(img) = from_bytes(&buf[i..], "paste.png") {
                                return Ok(Some(img));
                            }
                        }
                        debug!("PNG format present but decode failed ({} bytes)", buf.len());
                    }
                    Ok(_) => debug!("PNG format empty"),
                    Err(e) => debug!("PNG get_vec: {e}"),
                }
            }
        }
    }

    // 2) JPEG
    for name in ["JFIF", "JPEG", "image/jpeg"] {
        if let Some(fmt) = register_format(name) {
            if is_format_avail(fmt.get()) {
                let mut buf = Vec::new();
                if raw::get_vec(fmt.get(), &mut buf).is_ok() && !buf.is_empty() {
                    if let Ok(img) = from_bytes(&buf, "paste.jpg") {
                        return Ok(Some(img));
                    }
                }
            }
        }
    }

    // 3) CF_DIB / CF_DIBV5 — preferred over CF_BITMAP for Snipping Tool / screenshots.
    //    DIB is a memory blob; BITMAP is an HBITMAP handle that often fails to serialize.
    for fmt in [formats::CF_DIB, formats::CF_DIBV5] {
        if !is_format_avail(fmt) {
            continue;
        }
        let mut buf = Vec::new();
        match raw::get_vec(fmt, &mut buf) {
            Ok(_) if !buf.is_empty() => {
                if let Some(img) = dib_to_pending(&buf) {
                    return Ok(Some(img));
                }
                debug!("DIB fmt={fmt} len={} decode failed", buf.len());
            }
            Ok(_) => {}
            Err(e) => debug!("DIB fmt={fmt}: {e}"),
        }
    }

    // 4) CF_BITMAP via clipboard-win (produces a BMP file buffer)
    if is_format_avail(formats::CF_BITMAP) {
        let mut buf = Vec::new();
        match Bitmap.read_clipboard(&mut buf) {
            Ok(_) if !buf.is_empty() => {
                if let Ok(img) = from_bytes(&buf, "paste.bmp") {
                    return Ok(Some(img));
                }
                if buf.len() > 14 && buf.starts_with(b"BM") {
                    if let Some(img) = dib_to_pending(&buf[14..]) {
                        return Ok(Some(img));
                    }
                }
                if let Some(img) = dib_to_pending(&buf) {
                    return Ok(Some(img));
                }
                debug!("CF_BITMAP {} bytes decode failed", buf.len());
            }
            Ok(_) => debug!("CF_BITMAP empty"),
            Err(e) => debug!("CF_BITMAP read: {e}"),
        }
    }

    // 5) File list (Explorer copy)
    if is_format_avail(formats::CF_HDROP) {
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        if FileList.read_clipboard(&mut paths).is_ok() {
            for path in paths {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if matches!(
                    ext.as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff"
                ) {
                    if let Ok(img) = from_path(&path) {
                        return Ok(Some(img));
                    }
                }
            }
        }
    }

    // 6) HTML / Unicode text (data URI, file path, <img src>)
    if let Some(fmt) = register_format("HTML Format") {
        if is_format_avail(fmt.get()) {
            let mut buf = Vec::new();
            if raw::get_vec(fmt.get(), &mut buf).is_ok() {
                let s = String::from_utf8_lossy(&buf);
                if let Some(img) = from_paste_payload(&s) {
                    return Ok(Some(img));
                }
            }
        }
    }
    let mut text = String::new();
    if Unicode.read_clipboard(&mut text).is_ok() {
        if let Some(img) = from_paste_payload(&text) {
            return Ok(Some(img));
        }
    }

    Ok(None)
}

/// Decode CF_DIB / CF_DIBV5: manual 24/32bpp first (screenshots), then image crate.
#[cfg(windows)]
fn dib_to_pending(dib: &[u8]) -> Option<PendingImage> {
    use image::codecs::bmp::BmpDecoder;

    if dib.len() < 40 {
        return None;
    }

    // Fast path: Win+Shift+S / Snipping Tool → 32bpp BI_RGB or BI_BITFIELDS
    if let Some((w, h, rgba)) = decode_dib_pixels(dib) {
        if let Ok(img) = from_rgba(rgba, w, h, "paste.png") {
            return Some(img);
        }
    }

    let mut data = dib.to_vec();
    tweak_dib_header_for_decoder(&mut data);

    if let Ok(decoder) = BmpDecoder::new_without_file_header(std::io::Cursor::new(&data)) {
        if let Ok(dyn_img) = image::DynamicImage::from_decoder(decoder) {
            let rgba = dyn_img.to_rgba8();
            let (w, h) = rgba.dimensions();
            return from_rgba(rgba.into_raw(), w, h, "paste.png").ok();
        }
    }

    if let Some(bmp) = dib_to_bmp_file(&data) {
        if let Ok(img) = from_bytes(&bmp, "paste.bmp") {
            return Some(img);
        }
    }

    debug!(
        "DIB decode failed len={} hdr={:?}",
        dib.len(),
        dib.get(0..16)
    );
    None
}

/// Manual DIB → RGBA for common screenshot formats (24/32 bpp, BI_RGB / BI_BITFIELDS).
#[cfg(windows)]
fn decode_dib_pixels(dib: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if !(40..=256).contains(&header_size) || header_size > dib.len() {
        return None;
    }
    let width = i32::from_le_bytes(dib[4..8].try_into().ok()?);
    let height_raw = i32::from_le_bytes(dib[8..12].try_into().ok()?);
    let planes = u16::from_le_bytes(dib[12..14].try_into().ok()?);
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    if planes != 1 || width <= 0 || width > 16_384 {
        return None;
    }
    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs();
    if height == 0 || height > 16_384 {
        return None;
    }
    // BI_RGB=0, BI_BITFIELDS=3
    if compression != 0 && compression != 3 {
        return None;
    }
    if bit_count != 24 && bit_count != 32 {
        return None;
    }

    let mut pixel_offset = header_size;
    if compression == 3 && header_size == 40 {
        // 3 masks after BITMAPINFOHEADER (alpha optional / rare for CF_DIB)
        pixel_offset += 12;
    }
    if bit_count <= 8 {
        let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
        let n = if colors_used != 0 {
            colors_used
        } else {
            1usize << bit_count
        };
        pixel_offset += n * 4;
    }
    if pixel_offset >= dib.len() {
        return None;
    }
    let pixels = &dib[pixel_offset..];
    let w = width as usize;
    let h = height as usize;
    let bpp = (bit_count / 8) as usize;
    let stride = ((w * bpp + 3) / 4) * 4;
    let need = stride.checked_mul(h)?;
    if pixels.len() < need {
        debug!(
            "DIB pixels short: have={} need={} {}x{} bpp={}",
            pixels.len(),
            need,
            w,
            h,
            bit_count
        );
        return None;
    }

    let mut rgba = vec![0u8; w * h * 4];
    for row in 0..h {
        let src_row = if top_down { row } else { h - 1 - row };
        let src = &pixels[src_row * stride..src_row * stride + w * bpp];
        let dst_off = row * w * 4;
        if bit_count == 32 {
            for x in 0..w {
                let i = x * 4;
                let o = dst_off + x * 4;
                // Windows DIB is BGRA
                rgba[o] = src[i + 2];
                rgba[o + 1] = src[i + 1];
                rgba[o + 2] = src[i];
                // Many screenshots leave alpha=0; force opaque unless clearly used.
                let a = src[i + 3];
                rgba[o + 3] = if a == 0 { 255 } else { a };
            }
        } else {
            for x in 0..w {
                let i = x * 3;
                let o = dst_off + x * 4;
                rgba[o] = src[i + 2];
                rgba[o + 1] = src[i + 1];
                rgba[o + 2] = src[i];
                rgba[o + 3] = 255;
            }
        }
    }
    Some((width as u32, height, rgba))
}

#[cfg(windows)]
fn tweak_dib_header_for_decoder(dib: &mut [u8]) {
    if dib.len() < 40 {
        return;
    }
    let header_size = u32::from_le_bytes([dib[0], dib[1], dib[2], dib[3]]) as usize;
    let bit_count = u16::from_le_bytes([dib[14], dib[15]]);
    let compression = u32::from_le_bytes([dib[16], dib[17], dib[18], dib[19]]);
    if bit_count == 32 && compression == 0 {
        dib[16..20].copy_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
        if header_size >= 56 && dib.len() >= 56 {
            let r = u32::from_le_bytes([dib[40], dib[41], dib[42], dib[43]]);
            let g = u32::from_le_bytes([dib[44], dib[45], dib[46], dib[47]]);
            let b = u32::from_le_bytes([dib[48], dib[49], dib[50], dib[51]]);
            if r == 0 && g == 0 && b == 0 {
                dib[40..44].copy_from_slice(&0x00ff_0000u32.to_le_bytes());
                dib[44..48].copy_from_slice(&0x0000_ff00u32.to_le_bytes());
                dib[48..52].copy_from_slice(&0x0000_00ffu32.to_le_bytes());
                if header_size >= 60 && dib.len() >= 60 {
                    dib[52..56].copy_from_slice(&0xff00_0000u32.to_le_bytes());
                }
            }
        }
    }
}

#[cfg(windows)]
fn dib_to_bmp_file(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if header_size < 40 || header_size > dib.len() {
        return None;
    }
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let mut pixel_offset = 14 + header_size;
    if bit_count <= 8 {
        let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
        let n = if colors_used != 0 {
            colors_used
        } else {
            1usize << bit_count
        };
        pixel_offset += n * 4;
    } else if compression == 3 && header_size == 40 {
        pixel_offset += 12;
    }
    let mut out = Vec::with_capacity(14 + dib.len());
    out.extend_from_slice(b"BM");
    let file_size = (14 + dib.len()) as u32;
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

/// Read plain text from the clipboard (for our own paste path).
#[cfg(windows)]
pub fn clipboard_text() -> Option<String> {
    use clipboard_win::formats::Unicode;
    use clipboard_win::{Clipboard, Getter};
    let _clip = Clipboard::new_attempts(10).ok()?;
    let mut text = String::new();
    Unicode.read_clipboard(&mut text).ok()?;
    if text.is_empty() {
        None
    } else {
        Some(text.replace("\r\n", "\n"))
    }
}

#[cfg(not(windows))]
pub fn clipboard_text() -> Option<String> {
    None
}

/// Write plain text to the system clipboard (message "复制" etc.).
/// Uses clipboard-win on Windows so we don't depend on egui-winit/arboard.
pub fn set_clipboard_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    #[cfg(windows)]
    {
        use clipboard_win::formats::Unicode;
        use clipboard_win::{Clipboard, Setter};
        // Retry: another app may hold the lock briefly
        for _ in 0..8 {
            if let Ok(_clip) = Clipboard::new_attempts(20) {
                if Unicode.write_clipboard(&text.replace('\n', "\r\n")).is_ok() {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        false
    }
}

#[cfg(all(test, windows))]
mod dib_tests {
    use super::*;

    /// 2×2 top-down 32bpp BI_RGB DIB (BGRA pixels).
    fn sample_dib_32() -> Vec<u8> {
        let mut dib = vec![0u8; 40 + 2 * 2 * 4];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes()); // biSize
        dib[4..8].copy_from_slice(&2i32.to_le_bytes()); // width
        dib[8..12].copy_from_slice(&(-2i32).to_le_bytes()); // top-down height
        dib[12..14].copy_from_slice(&1u16.to_le_bytes()); // planes
        dib[14..16].copy_from_slice(&32u16.to_le_bytes()); // bit count
        // pixels: red, green / blue, white  (B,G,R,A)
        let px = [
            0u8, 0, 255, 0, // red (alpha 0 → forced opaque)
            0, 255, 0, 0, // green
            255, 0, 0, 0, // blue
            255, 255, 255, 0, // white
        ];
        dib[40..].copy_from_slice(&px);
        dib
    }

    #[test]
    fn manual_dib_32_decodes() {
        let (w, h, rgba) = decode_dib_pixels(&sample_dib_32()).expect("decode");
        assert_eq!((w, h), (2, 2));
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]); // red
        assert_eq!(&rgba[4..8], &[0, 255, 0, 255]); // green
        assert_eq!(&rgba[8..12], &[0, 0, 255, 255]); // blue
        assert_eq!(&rgba[12..16], &[255, 255, 255, 255]); // white
    }

    #[test]
    fn dib_to_pending_roundtrip() {
        let img = dib_to_pending(&sample_dib_32()).expect("pending");
        assert_eq!((img.width, img.height), (2, 2));
        assert!(img.png_bytes.starts_with(b"\x89PNG"));
    }
}
