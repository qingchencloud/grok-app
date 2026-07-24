//! Diagnose clipboard image reading.
//!
//! Usage:
//!   1. Win+Shift+S 截图，或在图片上右键「复制」/ 资源管理器复制 .png
//!   2. 立刻在本终端运行: cargo run --example clip_probe
//!   3. 不要先在别处 Ctrl+V 把剪贴板清掉
use grok_app::attachments;

fn main() {
    println!("=== GrokApp clipboard probe ===");
    println!(
        "hint (image formats advertised) = {}",
        attachments::clipboard_has_image_hint()
    );

    match attachments::from_clipboard_ex() {
        Ok(Some(img)) => {
            println!(
                "RESULT: OK image {}x{} png_bytes={} name={}",
                img.width,
                img.height,
                img.png_bytes.len(),
                img.name
            );
        }
        Ok(None) => println!("RESULT: no image decoded (formats empty or unsupported)"),
        Err(e) => println!("RESULT: ERR {e}"),
    }

    #[cfg(windows)]
    {
        use clipboard_win::formats::{self, Unicode};
        use clipboard_win::{
            count_formats, is_format_avail, raw, register_format, Clipboard, EnumFormats, Getter,
        };

        match Clipboard::new_attempts(10) {
            Ok(_c) => {
                let n = count_formats().unwrap_or(0);
                println!("OpenClipboard: ok  format_count={n}");
                if n == 0 {
                    println!();
                    println!(">>> 剪贴板是空的。请先截图/复制图片，再立刻重跑本命令。");
                    println!("    Win+Shift+S → 框选 → 再运行: cargo run --example clip_probe");
                    return;
                }

                println!(
                    "std: CF_BITMAP={} CF_DIB={} CF_DIBV5={} CF_HDROP={} CF_UNICODETEXT={}",
                    is_format_avail(formats::CF_BITMAP),
                    is_format_avail(formats::CF_DIB),
                    is_format_avail(formats::CF_DIBV5),
                    is_format_avail(formats::CF_HDROP),
                    is_format_avail(formats::CF_UNICODETEXT),
                );
                for name in ["PNG", "JFIF", "JPEG", "GIF", "HTML Format"] {
                    if let Some(f) = register_format(name) {
                        println!(
                            "  named {name:?} id={} avail={}",
                            f.get(),
                            is_format_avail(f.get())
                        );
                    }
                }

                print!("all format ids: ");
                let mut any = false;
                for fmt in EnumFormats::new() {
                    print!("{fmt} ");
                    any = true;
                }
                if !any {
                    print!("(none)");
                }
                println!();

                // Try each image path briefly
                if is_format_avail(formats::CF_DIB) {
                    let mut buf = Vec::new();
                    match raw::get_vec(formats::CF_DIB, &mut buf) {
                        Ok(n) => println!("CF_DIB get_vec: n={n} len={}", buf.len()),
                        Err(e) => println!("CF_DIB get_vec: err {e}"),
                    }
                }
                if let Some(f) = register_format("PNG") {
                    if is_format_avail(f.get()) {
                        let mut buf = Vec::new();
                        match raw::get_vec(f.get(), &mut buf) {
                            Ok(n) => println!(
                                "PNG get_vec: n={n} len={} magic={:?}",
                                buf.len(),
                                buf.get(0..8)
                            ),
                            Err(e) => println!("PNG get_vec: err {e}"),
                        }
                    }
                }

                let mut text = String::new();
                if Unicode.read_clipboard(&mut text).is_ok() && !text.is_empty() {
                    let preview: String = text.chars().take(80).collect();
                    println!("text preview: {preview:?} (len={})", text.len());
                }
            }
            Err(e) => {
                println!("OpenClipboard failed: {e}");
                println!(
                    "(another app may be holding the clipboard — close Snipping Tool / retry)"
                );
            }
        }
    }

    println!();
    println!("Done. If RESULT was OK, Ctrl+V in GrokApp should attach the image.");
}
