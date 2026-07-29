//! Product sharing links and localized copy.

use crate::i18n::Locale;

pub const HOMEPAGE_URL: &str = "https://qingchencloud.github.io/grok-app/";
pub const DOWNLOAD_URL: &str = "https://github.com/qingchencloud/grok-app/releases/latest";

pub fn share_text(locale: Locale) -> String {
    match locale {
        Locale::Zh => format!(
            "推荐 Grok Desktop：官方 Grok CLI 的原生可视化客户端，无需终端操作即可管理项目、聊天会话和工具调用。\n\n项目主页：{HOMEPAGE_URL}\n下载：{DOWNLOAD_URL}"
        ),
        Locale::En => format!(
            "Meet Grok Desktop — a native visual client for the official Grok CLI. Manage projects, chat sessions, and tool runs without terminal operations.\n\nProject: {HOMEPAGE_URL}\nDownload: {DOWNLOAD_URL}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_share_copy_contains_public_links() {
        for locale in [Locale::En, Locale::Zh] {
            let text = share_text(locale);
            assert!(text.contains(HOMEPAGE_URL));
            assert!(text.contains(DOWNLOAD_URL));
            assert!(text.contains("Grok Desktop"));
        }
    }
}
