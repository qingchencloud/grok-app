//! Desktop toast notifications (task-complete, etc.).

use notify_rust::Notification;

/// Best-effort system notification. Never panics; failures are logged.
pub fn notify(title: &str, body: &str) {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() && body.is_empty() {
        return;
    }
    let mut n = Notification::new();
    n.summary(if title.is_empty() {
        "Grok Desktop"
    } else {
        title
    })
    .body(if body.is_empty() { " " } else { body })
    .appname("Grok Desktop")
    .timeout(notify_rust::Timeout::Milliseconds(6000));

    // Icon: optional path next to install / assets — ignore if missing
    if let Some(icon) = find_notify_icon() {
        n.icon(&icon);
    }

    if let Err(e) = n.show() {
        tracing::debug!("desktop notification failed: {e:#}");
    }
}

fn find_notify_icon() -> Option<String> {
    // Prefer installed icon beside the exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["icon.png", "GrokDesktop.png", "logo.png"] {
                let p = dir.join(name);
                if p.is_file() {
                    return Some(p.display().to_string());
                }
            }
        }
    }
    None
}

/// Turn-complete toast content (localized by caller).
pub fn notify_turn_done(title: &str, detail: &str) {
    notify(title, detail);
}
