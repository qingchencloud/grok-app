//! Manual clipboard dump — open once, read formats without nested OpenClipboard.
fn main() {
    #[cfg(windows)]
    {
        use clipboard_win::formats::{self, Bitmap};
        use clipboard_win::{is_format_avail, raw, register_format, Clipboard, Getter};

        let _c = Clipboard::new_attempts(10).expect("OpenClipboard");
        println!(
            "BITMAP={} DIB={} DIBV5={}",
            is_format_avail(formats::CF_BITMAP),
            is_format_avail(formats::CF_DIB),
            is_format_avail(formats::CF_DIBV5)
        );

        // While open: use read_clipboard / raw::get — never get_clipboard().
        let mut bmp = Vec::new();
        match Bitmap.read_clipboard(&mut bmp) {
            Ok(n) => {
                println!("Bitmap n={n} bytes={} magic={:?}", bmp.len(), bmp.get(0..4));
                let _ = std::fs::write("target/clip_bitmap.bin", &bmp);
            }
            Err(e) => println!("Bitmap err: {e}"),
        }

        // DIB first (before Bitmap) — get_vec grows the buffer; raw::get needs pre-size.
        for fmt in [formats::CF_DIB, formats::CF_DIBV5] {
            let mut buf = Vec::new();
            match raw::get_vec(fmt, &mut buf) {
                Ok(n) => {
                    println!("fmt {fmt} n={n} len={}", buf.len());
                    if !buf.is_empty() {
                        println!(
                            "  header biSize={:?} w={:?} h={:?} bpp={:?}",
                            buf.get(0..4),
                            buf.get(4..8),
                            buf.get(8..12),
                            buf.get(14..16)
                        );
                        let _ = std::fs::write(format!("target/clip_{fmt}.bin"), &buf);
                    }
                }
                Err(e) => println!("fmt {fmt} err: {e}"),
            }
        }

        if let Some(f) = register_format("PNG") {
            let mut buf = Vec::new();
            match raw::get_vec(f.get(), &mut buf) {
                Ok(n) => println!("PNG n={n} len={} magic={:?}", buf.len(), buf.get(0..8)),
                Err(e) => println!("PNG err: {e}"),
            }
        }
    }
}
