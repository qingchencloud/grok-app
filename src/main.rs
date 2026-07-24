// Release: no console window. Debug: keep console for logs/panic.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use grok_app::{AppConfig, GrokApp};
use std::io::Write;
use std::panic;
use std::path::PathBuf;

fn crash_log_path() -> PathBuf {
    AppConfig::config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("crash.log")
}

/// Drop noisy arboard "image-only clipboard" paste errors from egui-winit.
fn install_quiet_log_filter() -> Result<(), log::SetLoggerError> {
    struct QuietArboard;
    impl log::Log for QuietArboard {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let msg = record.args().to_string();
            if msg.contains("arboard paste error")
                || msg.contains("clipboard contents were not available")
            {
                return;
            }
            // Keep real warnings/errors visible on stderr
            if record.level() <= log::Level::Warn {
                eprintln!("[{}] {}", record.level(), msg);
            }
        }
        fn flush(&self) {}
    }
    log::set_logger(&QuietArboard).map(|()| log::set_max_level(log::LevelFilter::Warn))
}

fn append_crash_log(msg: &str) {
    let path = crash_log_path();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            f,
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            msg
        );
    }
    eprintln!("[GrokApp] {msg}");
}

fn main() {
    panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<Any>".into()
        };
        append_crash_log(&format!("PANIC at {loc}: {payload}"));
    }));

    // Log to file even if console is closed
    let log_path = crash_log_path();
    let _ = std::fs::create_dir_all(log_path.parent().unwrap_or(std::path::Path::new(".")));

    // try_init: never panic if a global subscriber/logger is already installed
    // (second launch / hot reload edge cases).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();

    // egui-winit calls arboard.get_text() on every Ctrl+V and log::error!s when
    // the clipboard is image-only. We handle image paste ourselves; silence that noise.
    let _ = install_quiet_log_filter();

    append_crash_log("startup: begin");

    let config = AppConfig::load();
    append_crash_log(&format!(
        "startup: config ok cwd={} model={} size={}x{}",
        config.cwd, config.model, config.window_width, config.window_height
    ));

    // Prefer OpenGL (glow) — wgpu crashes on some Windows GPU/driver combos
    // and looks exactly like a "flash exit".
    // Default size is large enough that sidebar + chat + composer fit on
    // typical 1080p / 125% DPI desktops without clipping the input bar.
    // Enforce a floor so older configs with a short height still show the composer.
    let win_w = config.window_width.clamp(1000.0, 2400.0);
    let win_h = config.window_height.clamp(760.0, 1600.0);

    // App icon from official Grok mark (reference/grok-app icons).
    let app_icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([win_w, win_h])
        .with_min_inner_size([880.0, 640.0])
        .with_title("Grok")
        .with_active(true)
        .with_visible(true)
        .with_decorations(true)
        .with_transparent(false)
        .with_maximized(false);
    if let Some(icon) = app_icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        centered: true,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    append_crash_log("startup: calling run_native (renderer=Glow)");

    let result = eframe::run_native(
        "Grok Desktop",
        options,
        Box::new(|cc| {
            append_crash_log("startup: CreationContext");
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let app =
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| GrokApp::new(cc))) {
                    Ok(app) => app,
                    Err(e) => {
                        append_crash_log(&format!("GrokApp::new panicked: {e:?}"));
                        return Err(format!("init panic: {e:?}").into());
                    }
                };
            append_crash_log("startup: GrokApp ready — window should be visible");
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    );

    match result {
        Ok(()) => append_crash_log("shutdown: clean exit"),
        Err(e) => {
            append_crash_log(&format!("run_native FAILED: {e}"));
            eprintln!("\nGrokApp 启动失败: {e}");
            eprintln!("日志: {}", crash_log_path().display());
            // Keep console open when double-clicked via bat
            #[cfg(windows)]
            {
                eprintln!("按回车键退出…");
                let mut s = String::new();
                let _ = std::io::stdin().read_line(&mut s);
            }
            std::process::exit(1);
        }
    }
}
