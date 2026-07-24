//! UI strings — English primary, Chinese secondary.
//!
//! Call [`set_locale`] when config loads / user switches language, then [`t`]
//! from any UI code. Default is English.

use std::sync::atomic::{AtomicU8, Ordering};

static LOCALE: AtomicU8 = AtomicU8::new(0); // 0 = En, 1 = Zh

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En = 0,
    Zh = 1,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_cn" | "chinese" | "cn" => Self::Zh,
            _ => Self::En,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Zh => "中文",
        }
    }
}

pub fn set_locale(locale: Locale) {
    LOCALE.store(locale as u8, Ordering::Relaxed);
}

pub fn current_locale() -> Locale {
    match LOCALE.load(Ordering::Relaxed) {
        1 => Locale::Zh,
        _ => Locale::En,
    }
}

#[inline]
pub fn t() -> &'static Strings {
    match current_locale() {
        Locale::Zh => &ZH,
        Locale::En => &EN,
    }
}

// ── Parameterized helpers ───────────────────────────────────────────────────

pub fn session_label(short_id: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("会话 {short_id}"),
        Locale::En => format!("Session {short_id}"),
    }
}

pub fn new_session_status(cwd_short: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("新建会话 · {cwd_short}"),
        Locale::En => format!("New session · {cwd_short}"),
    }
}

pub fn generating_with_n(n: usize) -> String {
    match current_locale() {
        Locale::Zh => format!("生成中…（{n} 个附件）"),
        Locale::En => format!("Generating… ({n} attachments)"),
    }
}

pub fn max_attachments(n: usize) -> String {
    match current_locale() {
        Locale::Zh => format!("最多附加 {n} 个附件"),
        Locale::En => format!("At most {n} attachments"),
    }
}

pub fn attached_progress(cur: usize, max: usize, n: usize) -> String {
    match current_locale() {
        Locale::Zh => format!("已附加 {cur}/{max} · 图{n}"),
        Locale::En => format!("Attached {cur}/{max} · image {n}"),
    }
}

pub fn image_n_path(n: usize, path: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("图{n}: {path}"),
        Locale::En => format!("Image {n}: {path}"),
    }
}

pub fn tool_status(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("工具 · {title}"),
        Locale::En => format!("Tool · {title}"),
    }
}

pub fn tool_done_status(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("工具完成 · {title}"),
        Locale::En => format!("Tool done · {title}"),
    }
}

pub fn tool_running_status(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("工具执行中 · {title}"),
        Locale::En => format!("Tool running · {title}"),
    }
}

pub fn waiting_permission(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("等待权限 · {title}"),
        Locale::En => format!("Awaiting permission · {title}"),
    }
}

pub fn auto_approved(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("自动批准 · {title}"),
        Locale::En => format!("Auto-approved · {title}"),
    }
}

pub fn finished_with(reason: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("完成 · {reason}"),
        Locale::En => format!("Done · {reason}"),
    }
}

pub fn context_tooltip(
    used: &str,
    win: &str,
    src: &str,
    model: &str,
    effort: &str,
    extra: &str,
) -> String {
    match current_locale() {
        Locale::Zh => format!(
            "上下文 {used}/{win}\n窗口上限：{win}（{src}）\n模型 {model} · 强度 {effort}{extra}"
        ),
        Locale::En => format!(
            "Context {used}/{win}\nWindow: {win} ({src})\nModel {model} · effort {effort}{extra}"
        ),
    }
}

pub fn stuck_no_output_secs(s: u64) -> String {
    match current_locale() {
        Locale::Zh => format!("已 {s} 秒无新输出，可能卡住"),
        Locale::En => format!("No new output for {s}s — may be stuck"),
    }
}

pub fn stuck_running_secs(s: u64) -> String {
    match current_locale() {
        Locale::Zh => format!("本轮已运行 {s} 秒，仍未结束"),
        Locale::En => format!("Turn running for {s}s, still not finished"),
    }
}

pub fn archived_status(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("已归档 · {title}"),
        Locale::En => format!("Archived · {title}"),
    }
}

pub fn imported_status(title: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("已添加 · {title}"),
        Locale::En => format!("Added · {title}"),
    }
}

pub fn save_failed(e: impl std::fmt::Display) -> String {
    match current_locale() {
        Locale::Zh => format!("保存失败: {e}"),
        Locale::En => format!("Save failed: {e}"),
    }
}

pub fn err_cwd_missing(cwd: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("目录不存在: {cwd}"),
        Locale::En => format!("Directory not found: {cwd}"),
    }
}

pub fn connect_failed(e: impl std::fmt::Display) -> String {
    match current_locale() {
        Locale::Zh => format!("连接失败: {e:#}"),
        Locale::En => format!("Connect failed: {e:#}"),
    }
}

pub fn create_session_failed(e: impl std::fmt::Display) -> String {
    match current_locale() {
        Locale::Zh => format!("创建会话失败: {e:#}"),
        Locale::En => format!("Create session failed: {e:#}"),
    }
}

pub fn effort_chip(label: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("强度 · {label}"),
        Locale::En => format!("Effort · {label}"),
    }
}

pub fn model_chip(model: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("模型  {model}"),
        Locale::En => format!("Model  {model}"),
    }
}

pub fn effort_chip_full(effort: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("强度  {effort}"),
        Locale::En => format!("Effort  {effort}"),
    }
}

pub fn status_line(conn: &str, model: &str, effort: &str, ctx: &str, pid: &str) -> String {
    match current_locale() {
        Locale::Zh => {
            format!("状态 · {conn} · 模型 {model} · 强度 {effort} · 上下文 {ctx} · PID {pid}")
        }
        Locale::En => {
            format!("Status · {conn} · model {model} · effort {effort} · context {ctx} · PID {pid}")
        }
    }
}

pub fn paste_image_failed(formats: &str) -> String {
    match current_locale() {
        Locale::Zh => format!(
            "剪贴板图片读取失败（{formats}）。请再试 Ctrl+V，或用「＋ 附件」选文件 / 拖放图片。"
        ),
        Locale::En => format!(
            "Could not read clipboard image ({formats}). Try Ctrl+V again, or use Attach / drop a file."
        ),
    }
}

/// Bundled UI copy.
#[derive(Clone, Copy)]
pub struct Strings {
    // Chrome
    pub app_name: &'static str,
    pub settings: &'static str,
    pub logs: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
    pub reconnect: &'static str,
    pub stop: &'static str,
    pub force_end: &'static str,
    pub force_end_turn: &'static str,
    pub force_end_stuck: &'static str,
    pub new_chat: &'static str,
    pub search_sessions: &'static str,
    pub import: &'static str,
    pub import_tip: &'static str,
    pub archive: &'static str,
    pub archive_tip: &'static str,
    pub projects: &'static str,
    pub refresh_sessions: &'static str,
    pub agent_connected: &'static str,
    pub agent_disconnected: &'static str,
    pub day: &'static str,
    pub night: &'static str,
    pub day_tip: &'static str,
    pub night_tip: &'static str,
    pub login: &'static str,
    pub process: &'static str,
    pub ready: &'static str,
    pub connecting: &'static str,
    pub connecting_ellipsis: &'static str,
    pub not_connected: &'static str,
    pub offline: &'static str,
    pub generating: &'static str,
    pub generating_ellipsis: &'static str,
    pub thinking_ellipsis: &'static str,
    pub planning_ellipsis: &'static str,
    pub cancelled: &'static str,
    pub error_status: &'static str,
    pub copy: &'static str,
    pub cancel: &'static str,
    pub confirm: &'static str,
    pub close: &'static str,
    pub save: &'static str,
    pub save_only: &'static str,
    pub save_and_reconnect: &'static str,
    pub reset: &'static str,
    pub clear: &'static str,
    pub browse: &'static str,
    pub choose_image: &'static str,
    pub attach: &'static str,
    pub attach_tip: &'static str,
    pub paste_image: &'static str,
    pub send: &'static str,
    pub input_placeholder: &'static str,
    pub input_connect_first: &'static str,
    pub input_optional_note: &'static str,
    pub composer_hint: &'static str,
    pub jump_prev: &'static str,
    pub jump_next: &'static str,
    pub jump_bottom: &'static str,
    pub jump_prev_tip: &'static str,
    pub jump_next_tip: &'static str,
    pub jump_bottom_tip: &'static str,
    pub show_all: &'static str,
    pub history_collapsed: &'static str,
    pub show_sidebar: &'static str,
    pub hide_sidebar: &'static str,
    pub model: &'static str,
    pub effort: &'static str,
    pub effort_heading: &'static str,
    pub working_dir: &'static str,
    pub session_reported: &'static str,
    pub not_read: &'static str,
    pub unknown: &'static str,
    pub no_agent_process: &'static str,
    pub drop_to_attach: &'static str,

    // Chat empty / tools
    pub empty_title: &'static str,
    pub empty_subtitle: &'static str,
    pub chip_explore: &'static str,
    pub chip_explore_hint: &'static str,
    pub chip_explore_prompt: &'static str,
    pub chip_fix: &'static str,
    pub chip_fix_hint: &'static str,
    pub chip_fix_prompt: &'static str,
    pub chip_implement: &'static str,
    pub chip_implement_hint: &'static str,
    pub chip_implement_prompt: &'static str,
    pub chip_docs: &'static str,
    pub chip_docs_hint: &'static str,
    pub chip_docs_prompt: &'static str,
    pub thinking: &'static str,
    pub thought_n: &'static str,
    pub plan: &'static str,
    pub detail: &'static str,
    pub tool_running: &'static str,
    pub tool_done: &'static str,
    pub tool_failed: &'static str,
    pub tool_cancelled: &'static str,
    pub tool_default: &'static str,
    pub phase_permission: &'static str,
    pub phase_thinking: &'static str,
    pub phase_processing: &'static str,
    pub phase_waiting: &'static str,
    pub organizing_reply: &'static str,
    pub default_user_name: &'static str,
    pub me: &'static str,

    // Session list
    pub no_sessions: &'static str,
    pub no_match_sessions: &'static str,
    pub click_new_chat: &'static str,
    pub new_chat_in_project: &'static str,
    pub expand: &'static str,
    pub collapse: &'static str,
    pub no_chats: &'static str,
    pub untitled: &'static str,
    pub open: &'static str,
    pub rename: &'static str,
    pub delete_index_disk: &'static str,
    pub delete_index_disk_tip: &'static str,

    // Dialogs
    pub rename_session: &'static str,
    pub session_title: &'static str,
    pub title_hint: &'static str,
    pub archived_sessions: &'static str,
    pub archived_hint: &'static str,
    pub no_archived: &'static str,
    pub restore: &'static str,
    pub delete: &'static str,
    pub delete_disk_tip: &'static str,
    pub import_sessions: &'static str,
    pub refresh: &'static str,
    pub filter_hint: &'static str,
    pub no_importable: &'static str,
    pub no_match: &'static str,
    pub untitled_session: &'static str,
    pub add: &'static str,
    pub new_chat_title: &'static str,
    pub new_chat_body: &'static str,
    pub will_use: &'static str,
    pub always_approve_chip: &'static str,
    pub pick_folder: &'static str,
    pub need_project_dir: &'static str,
    pub dir_missing_check: &'static str,
    pub quick_picks: &'static str,
    pub current_dir: &'static str,
    pub start_chat: &'static str,
    pub preview: &'static str,
    pub preview_fail: &'static str,

    // Activity
    pub act_current: &'static str,
    pub act_generating: &'static str,
    pub act_tool: &'static str,
    pub act_permission: &'static str,
    pub act_connecting: &'static str,

    // Settings tabs
    pub tab_appearance: &'static str,
    pub tab_agent: &'static str,
    pub tab_cli: &'static str,
    pub tab_advanced: &'static str,
    pub tab_about: &'static str,
    pub tab_appearance_desc: &'static str,
    pub tab_agent_desc: &'static str,
    pub tab_cli_desc: &'static str,
    pub tab_advanced_desc: &'static str,
    pub tab_about_desc: &'static str,
    pub language: &'static str,
    pub language_hint: &'static str,
    pub theme: &'static str,
    pub theme_hint: &'static str,
    pub profile: &'static str,
    pub profile_hint: &'static str,
    pub display_name: &'static str,
    pub display_name_hint: &'static str,
    pub avatar_image: &'static str,
    pub avatar_unset_hint: &'static str,
    pub font_size: &'static str,
    pub font_scale: &'static str,
    pub chat_experience: &'static str,
    pub smooth_stream: &'static str,
    pub smooth_stream_hint: &'static str,
    pub show_thoughts: &'static str,
    pub show_thoughts_hint: &'static str,
    pub expand_tools: &'static str,
    pub expand_tools_hint: &'static str,
    pub enter_to_send: &'static str,
    pub enter_to_send_hint: &'static str,
    pub model_hint: &'static str,
    pub select_model: &'static str,
    pub or_type_model: &'static str,
    pub effort_hint: &'static str,
    pub working_dir_hint: &'static str,
    pub permissions: &'static str,
    pub always_approve: &'static str,
    pub always_approve_hint: &'static str,
    pub auto_connect: &'static str,
    pub auto_connect_hint: &'static str,
    pub grok_binary: &'static str,
    pub grok_binary_hint: &'static str,
    pub auto_detect: &'static str,
    pub cli_status: &'static str,
    pub cli_status_hint: &'static str,
    pub cli_installed: &'static str,
    pub cli_missing: &'static str,
    pub install_update: &'static str,
    pub about_title: &'static str,
    pub about_body: &'static str,
    pub unofficial_notice: &'static str,
    pub settings_saved: &'static str,
    pub agent_settings_saved: &'static str,

    // Slash
    pub slash_new_title: &'static str,
    pub slash_new_desc: &'static str,
    pub slash_settings_title: &'static str,
    pub slash_settings_desc: &'static str,
    pub slash_status_title: &'static str,
    pub slash_status_desc: &'static str,
    pub slash_logs_title: &'static str,
    pub slash_logs_desc: &'static str,
    pub slash_yolo_title: &'static str,
    pub slash_yolo_desc: &'static str,
    pub slash_clear_title: &'static str,
    pub slash_clear_desc: &'static str,
    pub slash_compact_title: &'static str,
    pub slash_compact_desc: &'static str,

    // Status / banners
    pub status_cleared_view: &'static str,
    pub status_switched_dark: &'static str,
    pub status_switched_light: &'static str,
    pub err_save_failed: &'static str,
    pub err_not_connected: &'static str,
    pub err_busy: &'static str,
    pub err_need_cwd: &'static str,
    pub err_cwd_missing: &'static str,
    pub status_no_user_msg: &'static str,
    pub status_first_user_msg: &'static str,
    pub status_last_user_msg: &'static str,
    pub status_at_latest: &'static str,
    pub status_jumped_prev: &'static str,
    pub status_jumped_next: &'static str,
    pub status_disconnected: &'static str,
    pub status_connected: &'static str,
    pub status_stopped: &'static str,
    pub status_force_ended: &'static str,
    pub status_force_ended_ok: &'static str,
    pub status_renamed: &'static str,
    pub status_deleted: &'static str,
    pub status_restored: &'static str,
    pub status_deleted_archive: &'static str,
    pub status_reading_clipboard: &'static str,
    pub status_pasted_text: &'static str,
    pub status_ready: &'static str,
    pub status_login_opened: &'static str,
    pub yolo_on: &'static str,
    pub yolo_off: &'static str,
    pub cli_missing_banner: &'static str,
    pub auth_missing_hint: &'static str,
    pub install_start: &'static str,
    pub install_started: &'static str,
    pub effort_low: &'static str,
    pub effort_medium: &'static str,
    pub effort_high: &'static str,
    pub effort_low_full: &'static str,
    pub effort_medium_full: &'static str,
    pub effort_high_full: &'static str,
    pub images_filter: &'static str,
    pub name_chars_hint: &'static str,
    pub send_tip: &'static str,
    pub runtime_logs: &'static str,
    pub clear_logs: &'static str,
    pub tool_permission: &'static str,
    pub allow_once: &'static str,
    pub deny: &'static str,
    pub reinstall_update: &'static str,
    pub install_cli_once: &'static str,
    pub installing: &'static str,
    pub install_log: &'static str,
    pub status_refreshed: &'static str,
    pub extra_agent_args: &'static str,
    pub extra_agent_args_hint: &'static str,
    pub extra_args_example: &'static str,
    pub shared_with_tui: &'static str,
    pub save_config_toml: &'static str,
    pub reload: &'static str,
    pub unsaved: &'static str,
    pub local_sessions: &'static str,
    pub view_in_sidebar: &'static str,
    pub version: &'static str,
    pub auth_reuse_cli: &'static str,
    pub links: &'static str,
    pub ref_client: &'static str,
    pub capabilities: &'static str,
    pub save_settings: &'static str,
    pub slash_commands: &'static str,
    pub slash_plan_title: &'static str,
    pub slash_plan_desc: &'static str,
    pub slash_goal_title: &'static str,
    pub slash_goal_desc: &'static str,
    pub slash_help_title: &'static str,
    pub slash_help_desc: &'static str,
    // Settings — CLI / advanced residual
    pub current_effort: &'static str,
    pub logged_in: &'static str,
    pub not_logged_in: &'static str,
    pub cli_config_toml: &'static str,
    pub auto_compact_threshold: &'static str,
    pub yolo_global: &'static str,
    pub remember_tool_approvals: &'static str,
    pub load_envrc: &'static str,
    pub show_thinking_tui: &'static str,
    pub codebase_indexing: &'static str,
    pub cli_auto_update: &'static str,
    pub default_model: &'static str,
    pub permission_mode: &'static str,
    pub wrote_config_toml: &'static str,
    pub reloaded_from_disk: &'static str,
    pub sessions_count: &'static str,
    pub about_capabilities_body: &'static str,
    pub link_xai_cli: &'static str,
    pub link_grok_build: &'static str,
    pub untitled_paren: &'static str,
    pub new_session_title: &'static str,
    pub no_project: &'static str,
    pub unbound_cwd: &'static str,
    pub click_to_preview: &'static str,
    pub will_bind: &'static str,
    pub file_too_large: &'static str,
    pub decode_image_failed: &'static str,
    pub view_image_prompt: &'static str,
    pub grok_not_found: &'static str,
    pub title_empty: &'static str,
    pub not_in_app_index: &'static str,
    pub invalid_session_id: &'static str,
    pub install_ok: &'static str,
    pub install_done_refresh: &'static str,
    pub clipboard_busy: &'static str,
    pub image_read_failed: &'static str,
    pub pasted_image: &'static str,
    pub drop_image_failed: &'static str,
    pub approved_tool: &'static str,
    pub attach_save_failed: &'static str,
    pub attach_persist_failed: &'static str,
    pub archive_failed: &'static str,
    pub delete_failed: &'static str,
    pub rename_failed: &'static str,
    pub restore_failed: &'static str,
    pub import_failed: &'static str,
    pub prompt_failed: &'static str,
    pub thought_truncated: &'static str,
    pub tools_summary: &'static str,
    // Updates
    pub update_available: &'static str,
    pub update_check: &'static str,
    pub update_checking: &'static str,
    pub update_up_to_date: &'static str,
    pub update_modal_title: &'static str,
    pub update_later: &'static str,
    pub update_download: &'static str,
    pub update_open_releases: &'static str,
    pub update_changelog: &'static str,
    pub update_current: &'static str,
    pub update_latest: &'static str,
    pub update_failed: &'static str,
    pub update_badge: &'static str,
    pub update_view: &'static str,
    pub update_check_on_startup: &'static str,
    pub update_whats_new: &'static str,
    pub update_section: &'static str,
    pub version_label: &'static str,
}

macro_rules! s {
    ($($field:ident: $en:expr, $zh:expr),* $(,)?) => {
        const EN: Strings = Strings { $($field: $en),* };
        const ZH: Strings = Strings { $($field: $zh),* };
    };
}

s! {
    app_name: "Grok Desktop", "Grok Desktop",
    settings: "Settings", "设置",
    logs: "Logs", "日志",
    connect: "Connect", "连接",
    disconnect: "Disconnect", "断开",
    reconnect: "Connect / reconnect", "连接 / 重连",
    stop: "Stop", "停止",
    force_end: "Force end", "强制结束",
    force_end_turn: "Force end this turn", "强制结束本轮",
    force_end_stuck: "Force end", "强制结束",
    new_chat: "New chat", "新对话",
    search_sessions: "Search sessions…", "搜索会话…",
    import: "Import", "导入",
    import_tip: "Import CLI sessions into the App list", "从 CLI 导入会话到 App 列表",
    archive: "Archive", "归档",
    archive_tip: "View archived sessions", "查看已归档会话",
    projects: "Projects", "项目",
    refresh_sessions: "Refresh sessions", "刷新会话",
    agent_connected: "Agent connected", "Agent 已连接",
    agent_disconnected: "Agent disconnected", "Agent 未连接",
    day: "Light", "日间",
    night: "Dark", "夜间",
    day_tip: "Light appearance", "浅色外观",
    night_tip: "Dark appearance", "深色外观",
    login: "Login", "登录",
    process: "Process", "进程",
    ready: "Ready", "就绪",
    connecting: "Connecting", "连接中",
    connecting_ellipsis: "Connecting…", "正在连接…",
    not_connected: "Not connected", "未连接",
    offline: "Offline", "离线",
    generating: "Generating", "生成中",
    generating_ellipsis: "Generating…", "生成中…",
    thinking_ellipsis: "Thinking…", "思考中…",
    planning_ellipsis: "Planning…", "规划中…",
    cancelled: "Cancelled", "已取消",
    error_status: "Error", "出错",
    copy: "Copy", "复制",
    cancel: "Cancel", "取消",
    confirm: "Confirm", "确认",
    close: "Close", "关闭",
    save: "Save", "保存",
    save_only: "Save only", "仅保存",
    save_and_reconnect: "Save & reconnect", "保存并重连",
    reset: "Reset", "重置",
    clear: "Clear", "清除",
    browse: "Browse", "浏览",
    choose_image: "Choose image…", "选择图片…",
    attach: "Attach", "附件",
    attach_tip: "Attach · drop · Ctrl+V", "附件 · 拖放 · Ctrl+V",
    paste_image: "Paste image", "粘贴图",
    send: "Send", "发送",
    input_placeholder: "Message…", "输入消息…",
    input_connect_first: "Connect the agent to start chatting", "连接 Agent 后开始对话",
    input_optional_note: "Optional note…", "补充说明（可选）",
    composer_hint: "Enter send · Shift+Enter newline · Ctrl+↑ prev · Ctrl+V paste image",
        "Enter 发送 · Shift+Enter 换行 · Ctrl+↑ 上一条 · Ctrl+V 贴图",
    jump_prev: "Previous", "上一条",
    jump_next: "Next", "下一条",
    jump_bottom: "Bottom", "底部",
    jump_prev_tip: "Previous user message · Ctrl+↑", "上一条用户消息 · Ctrl+↑",
    jump_next_tip: "Next user message · Ctrl+↓", "下一条用户消息 · Ctrl+↓",
    jump_bottom_tip: "Scroll to latest", "滚到最新",
    show_all: "Show all", "显示全部",
    history_collapsed: "earlier messages folded", "条更早消息已折叠",
    show_sidebar: "Show sidebar", "显示侧栏",
    hide_sidebar: "Collapse sidebar", "收起侧栏",
    model: "Model", "模型",
    effort: "Effort", "强度",
    effort_heading: "Reasoning effort", "推理强度",
    working_dir: "Working directory", "工作目录",
    session_reported: "Session reported", "会话上报",
    not_read: "Not available", "未读取到",
    unknown: "Unknown", "未知",
    no_agent_process: "No agent process", "无 Agent 进程",
    drop_to_attach: "Drop to attach images", "松开以添加图片",

    empty_title: "What do you want to do?", "今天想做什么？",
    empty_subtitle: "Code · debug · ship", "写代码 · 查问题 · 改项目",
    chip_explore: "Explore project", "探索项目",
    chip_explore_hint: "Structure & purpose", "结构与用途",
    chip_explore_prompt: "Please overview the project structure and purpose of the current working directory",
        "请概览当前工作目录的项目结构与用途",
    chip_fix: "Fix issues", "修复问题",
    chip_fix_hint: "Build & errors", "编译与错误",
    chip_fix_prompt: "Analyze recent errors or build failures and suggest fixes",
        "请分析最近的错误或编译失败原因，并给出修复建议",
    chip_implement: "Implement feature", "实现功能",
    chip_implement_hint: "Plan then code", "方案再改代码",
    chip_implement_prompt: "Help me design and implement a small feature — explain the plan first, then change code",
        "帮我设计并实现一个简洁功能，先说明方案再改代码",
    chip_docs: "Write docs", "撰写文档",
    chip_docs_hint: "README draft", "README 草稿",
    chip_docs_prompt: "Generate a clear README.md draft from the current project",
        "根据当前项目生成一份清晰的 README.md 草稿",
    thinking: "Thinking", "思考",
    thought_n: "Thinking", "思考",
    plan: "Plan", "计划",
    detail: "Details", "详情",
    tool_running: "Running", "运行",
    tool_done: "Done", "完成",
    tool_failed: "Failed", "失败",
    tool_cancelled: "Cancelled", "取消",
    tool_default: "Running tool", "运行工具",
    phase_permission: "Permission required", "需要你确认工具权限",
    phase_thinking: "Reasoning", "模型推理中",
    phase_processing: "Working", "处理中",
    phase_waiting: "Waiting for agent", "等待 Agent 完成本轮",
    organizing_reply: "Composing reply…", "正在组织回复…",
    default_user_name: "Me", "我",
    me: "Me", "我",

    no_sessions: "No sessions yet", "暂无会话",
    no_match_sessions: "No matching sessions", "无匹配会话",
    click_new_chat: "Click “New chat” above to start", "点上方「新对话」开始",
    new_chat_in_project: "New chat in this project", "在此项目新建对话",
    expand: "Expand", "展开",
    collapse: "Collapse", "折叠",
    no_chats: "No chats", "暂无对话",
    untitled: "Untitled", "无标题",
    open: "Open", "打开",
    rename: "Rename…", "重命名…",
    delete_index_disk: "Delete (index + disk)", "删除（索引+磁盘）",
    delete_index_disk_tip: "Remove from App list and delete ~/.grok session folder",
        "从 App 列表移除，并删除 ~/.grok 会话目录",

    rename_session: "Rename session", "重命名会话",
    session_title: "Session title", "会话标题",
    title_hint: "New title", "输入新标题",
    archived_sessions: "Archive", "归档",
    archived_hint: "Archived sessions are hidden from the main list", "已归档会话不在主列表显示",
    no_archived: "No archived sessions", "暂无归档",
    restore: "Restore", "恢复",
    delete: "Delete", "删除",
    delete_disk_tip: "Remove from index and delete disk session", "从索引移除并删除磁盘会话",
    import_sessions: "Import sessions", "导入会话",
    refresh: "Refresh", "刷新",
    filter_hint: "Filter…", "筛选…",
    no_importable: "No importable sessions", "没有可导入的会话",
    no_match: "No matches", "无匹配结果",
    untitled_session: "Untitled session", "无标题会话",
    add: "Add", "添加",
    new_chat_title: "New chat", "新建对话",
    new_chat_body: "Pick a workspace. The agent reads, writes, and runs tools here.",
        "选择工作区，Agent 将在此目录读写与执行工具",
    will_use: "This session will use", "本次将使用",
    always_approve_chip: "Always approve", "自动批准",
    pick_folder: "Browse folder", "浏览文件夹",
    need_project_dir: "Enter or choose a project directory", "请填写或选择一个项目目录",
    dir_missing_check: "Directory does not exist — check the path", "目录不存在，请检查路径",
    quick_picks: "Quick picks", "快捷选择",
    current_dir: "Current folder", "当前目录",
    start_chat: "Start chat", "开始对话",
    preview: "Preview", "预览",
    preview_fail: "Could not load preview", "无法加载预览",

    act_current: "Current", "当前",
    act_generating: "Generating", "生成中",
    act_tool: "Tool", "工具中",
    act_permission: "Permission", "待权限",
    act_connecting: "Connecting", "连接中",

    tab_appearance: "Appearance", "外观",
    tab_agent: "Agent", "Agent",
    tab_cli: "CLI", "CLI",
    tab_advanced: "Advanced", "高级 / CLI",
    tab_about: "About", "关于",
    tab_appearance_desc: "Theme, profile, language, font size, chat behavior",
        "主题、个人资料、语言、字号与对话表现",
    tab_agent_desc: "Model, working directory, connection", "模型、工作目录与连接",
    tab_cli_desc: "Install official CLI and account", "安装官方 CLI 与账号",
    tab_advanced_desc: "Extra agent args and advanced options", "额外 agent 参数与高级选项",
    tab_about_desc: "Version and links", "版本与链接",
    language: "Language", "语言",
    language_hint: "UI language (English / 中文). Takes effect immediately.",
        "界面语言（English / 中文），立即生效",
    theme: "Theme", "主题",
    theme_hint: "Synced with the sidebar toggle", "与侧栏底部开关同步",
    profile: "Profile", "个人资料",
    profile_hint: "Name and avatar next to your messages; long names are truncated",
        "聊天区显示的名称与头像；过长名称会自动截断",
    display_name: "Display name", "显示名称",
    display_name_hint: "e.g. Me / Alex", "例如：我 / Alex",
    avatar_image: "Avatar image", "头像图片",
    avatar_unset_hint: "When empty, a letter badge from your name is used",
        "未设置时使用名称首字圆形徽章",
    font_size: "Font size", "字号",
    font_scale: "Scale", "缩放",
    chat_experience: "Chat experience", "对话体验",
    smooth_stream: "Smooth streaming", "流畅流式输出",
    smooth_stream_hint: "Reveal text gradually, closer to the CLI feel",
        "逐字显现，更接近 CLI 体感",
    show_thoughts: "Show thinking", "显示思考过程",
    show_thoughts_hint: "Show model reasoning / thought blocks",
        "折叠展示模型 reasoning / thought",
    expand_tools: "Expand tool details by default", "默认展开工具详情",
    expand_tools_hint: "Open tool call rows expanded", "工具调用行默认打开明细",
    enter_to_send: "Enter to send", "Enter 发送",
    enter_to_send_hint: "Off → Ctrl+Enter sends, Enter inserts a newline",
        "关闭后改为 Ctrl+Enter 发送，Enter 换行",
    model_hint: "Model id passed when starting the agent", "启动 agent 时使用的模型 id",
    select_model: "Select model", "选择模型",
    or_type_model: "Or type an id", "或手动输入",
    effort_hint: "Maps to CLI --reasoning-effort (low · medium · high)",
        "对应 CLI `--reasoning-effort`（low · medium · high）",
    working_dir_hint: "Default root for new chats and tool I/O", "新建对话与工具读写的默认根目录",
    permissions: "Permissions & connection", "权限与连接",
    always_approve: "Always approve tools", "自动批准工具",
    always_approve_hint: "Same as --always-approve; use on trusted machines",
        "等价 --always-approve，适合本机可信环境",
    auto_connect: "Connect on launch", "启动时自动连接",
    auto_connect_hint: "Start grok agent when the app opens", "打开应用后自动拉起 grok agent",
    grok_binary: "grok executable", "grok 可执行文件",
    grok_binary_hint: "Leave empty to auto-detect ~/.grok/bin and PATH",
        "留空则自动检测 ~/.grok/bin 与 PATH",
    auto_detect: "Auto-detect", "留空 = 自动检测",
    cli_status: "Status", "状态",
    cli_status_hint: "Desktop talks to the official CLI via grok agent stdio",
        "桌面端通过 grok agent stdio 对接官方 CLI",
    cli_installed: "Installed", "已安装",
    cli_missing: "CLI not found", "未检测到 CLI",
    install_update: "Install / update", "安装 / 更新",
    about_title: "About Grok Desktop", "关于 Grok Desktop",
    about_body: "Native GUI over the official Grok Build CLI (ACP). Sessions, tools, and auth stay with the CLI.",
        "官方 Grok Build CLI 的原生图形前端（ACP）。会话、工具与登录仍由 CLI 负责。",
    unofficial_notice: "Unofficial client — not affiliated with xAI.",
        "非官方客户端，与 xAI 无隶属关系。",
    settings_saved: "Settings saved", "设置已保存",
    agent_settings_saved: "Agent settings saved — reconnecting…", "Agent 设置已保存，正在重连…",

    slash_new_title: "New chat", "新对话",
    slash_new_desc: "Bind a working directory and start a session", "绑定工作目录并开始新会话",
    slash_settings_title: "Settings", "设置",
    slash_settings_desc: "Open settings", "打开设置面板",
    slash_status_title: "Status", "状态",
    slash_status_desc: "Connection / model / context summary", "显示连接 / 模型 / 上下文摘要",
    slash_logs_title: "Logs", "日志",
    slash_logs_desc: "Open runtime logs", "打开运行日志窗口",
    slash_yolo_title: "Toggle auto-approve", "切换自动批准",
    slash_yolo_desc: "Toggle --always-approve", "开/关 --always-approve",
    slash_clear_title: "Clear view", "清空当前视图",
    slash_clear_desc: "Clear local timeline (does not delete disk session)",
        "清空本机时间线（不删磁盘会话）",
    slash_compact_title: "Compact context", "压缩上下文",
    slash_compact_desc: "Send /compact to the agent if supported", "向 Agent 发送 /compact（若 CLI 支持）",

    status_cleared_view: "Cleared current view", "已清空当前视图",
    status_switched_dark: "Switched to dark mode", "已切换到夜间模式",
    status_switched_light: "Switched to light mode", "已切换到日间模式",
    err_save_failed: "Save failed", "保存失败",
    err_not_connected: "Agent not connected — connect from the sidebar first",
        "尚未连接 Agent，请先在侧栏连接",
    err_busy: "A turn is still running — press Stop before sending",
        "上一轮尚未结束，请先点「停止」再发送",
    err_need_cwd: "Please set a working directory", "请指定工作目录",
    err_cwd_missing: "Directory does not exist", "目录不存在",
    status_no_user_msg: "No user messages to jump to", "没有用户消息可跳转",
    status_first_user_msg: "Already at the first user message", "已是第一条用户消息",
    status_last_user_msg: "Already at the latest", "已是最新",
    status_at_latest: "At latest", "已到最新",
    status_jumped_prev: "Jumped to previous user message", "已定位到上一条用户消息",
    status_jumped_next: "Jumped to next user message", "已定位到下一条用户消息",
    status_disconnected: "Disconnected", "已断开",
    status_connected: "Connected", "已连接",
    status_stopped: "Stopped", "已停止",
    status_force_ended: "Force-ended", "已强制结束",
    status_force_ended_ok: "Force-ended · you can send again", "已强制结束 · 可继续发送",
    status_renamed: "Renamed", "已重命名",
    status_deleted: "Session deleted", "会话已删除",
    status_restored: "Restored from archive", "已从归档恢复",
    status_deleted_archive: "Archived session deleted", "已删除归档会话",
    status_reading_clipboard: "Reading clipboard…", "读取剪贴板…",
    status_pasted_text: "Pasted text", "已粘贴文字",
    status_ready: "Ready", "就绪",
    status_login_opened: "Opened grok login", "已打开 grok login",
    yolo_on: "Auto-approve tools enabled", "已开启自动批准工具",
    yolo_off: "Auto-approve tools disabled", "已关闭自动批准工具",
    cli_missing_banner: "grok CLI not found — install it from Settings",
        "未找到 grok CLI — 可在设置里一键安装",
    auth_missing_hint: "Tip: no ~/.grok/auth.json or XAI_API_KEY — use Login in the sidebar",
        "提示: 未检测到 ~/.grok/auth.json 或 XAI_API_KEY，可点侧栏「登录」",
    install_start: "Installing official CLI…", "开始安装官方 CLI…",
    install_started: "Install process started…", "安装进程已启动…",
    effort_low: "Low", "低",
    effort_medium: "Med", "中",
    effort_high: "High", "高",
    effort_low_full: "Low · faster", "低 · 更快",
    effort_medium_full: "Medium · balanced", "中 · 均衡",
    effort_high_full: "High · deeper", "高 · 更深",
    images_filter: "Images", "图片",
    name_chars_hint: "chars (normalized on save, max 24)", "字（保存时规范化，最多 24）",
    send_tip: "Send (Enter)", "发送 (Enter)",
    runtime_logs: "Runtime logs", "运行日志",
    clear_logs: "Clear", "清空",
    tool_permission: "Tool permission", "工具权限",
    allow_once: "Allow once", "允许一次",
    deny: "Deny", "拒绝",
    reinstall_update: "Reinstall / update", "重新安装 / 更新",
    install_cli_once: "Install CLI", "一键安装 CLI",
    installing: "Installing…", "安装中…",
    install_log: "Install log", "安装日志",
    status_refreshed: "Status refreshed", "状态已刷新",
    extra_agent_args: "Extra agent args", "额外 Agent 参数",
    extra_agent_args_hint: "Space-separated, inserted before `grok agent … stdio`",
        "空格分隔，插入到 `grok agent … stdio` 之前",
    extra_args_example: "e.g. --verbose", "例: --verbose",
    shared_with_tui: "Shared with the terminal TUI; save writes ~/.grok/config.toml",
        "与终端 TUI 共用；保存后写回 ~/.grok/config.toml",
    save_config_toml: "Save config.toml", "保存 config.toml",
    reload: "Reload", "重新加载",
    unsaved: "Unsaved", "未保存",
    local_sessions: "Local sessions", "本机会话",
    view_in_sidebar: "View in sidebar", "在侧栏查看",
    version: "Version", "版本",
    auth_reuse_cli: "Auth and sessions use the official CLI (~/.grok)",
        "认证与会话复用官方 CLI (~/.grok)",
    links: "Links", "链接",
    ref_client: "Reference client RongleCat/grok-app", "参考客户端 RongleCat/grok-app",
    capabilities: "This client", "本客户端能力",
    save_settings: "Save settings", "保存设置",
    slash_commands: "Commands", "指令",
    slash_plan_title: "Plan mode", "计划模式",
    slash_plan_desc: "Insert /plan hint", "插入 /plan 提示",
    slash_goal_title: "Goal mode", "目标模式",
    slash_goal_desc: "Insert /goal hint", "插入 /goal 提示",
    slash_help_title: "Help", "帮助",
    slash_help_desc: "List slash commands", "列出可用斜杠指令",
    current_effort: "Current", "当前",
    logged_in: "Logged in (~/.grok/auth.json)", "已登录 (~/.grok/auth.json)",
    not_logged_in: "Not logged in — use Login below", "未登录 — 请使用下方登录",
    cli_config_toml: "CLI config.toml", "CLI config.toml",
    auto_compact_threshold: "Auto-compact threshold %", "自动压缩阈值 %",
    yolo_global: "yolo (global always-approve)", "yolo（全局自动批准）",
    remember_tool_approvals: "Remember tool approvals", "记住工具批准",
    load_envrc: "Load .envrc", "加载 .envrc",
    show_thinking_tui: "Show thinking blocks in TUI", "TUI 显示思考块",
    codebase_indexing: "Codebase indexing", "代码库索引",
    cli_auto_update: "CLI auto-update", "CLI 自动更新",
    default_model: "Default model", "默认模型",
    permission_mode: "permission_mode", "permission_mode",
    wrote_config_toml: "Wrote ~/.grok/config.toml", "已写入 ~/.grok/config.toml",
    reloaded_from_disk: "Reloaded from disk", "已从磁盘重新加载",
    sessions_count: "sessions · ~/.grok/sessions", "条 · ~/.grok/sessions",
    about_capabilities_body: "• Streaming chat / thinking / tools\n\
• Images: Ctrl+V · attach · drag-drop\n\
• Sessions grouped by project folder\n\
• Light / dark · system title bar\n\
• New chat binds a working directory",
        "• 流式对话 / 思考过程 / 工具调用\n\
• 图片：Ctrl+V · 附件 · 拖放\n\
• 会话按项目目录分组\n\
• 日间 / 夜间 · 系统标题栏跟随\n\
• 新建对话可绑定工作目录",
    link_xai_cli: "xAI CLI install", "xAI CLI 安装",
    link_grok_build: "Grok Build", "Grok Build",
    update_available: "Update available", "发现新版本",
    update_check: "Check for updates", "检查更新",
    update_checking: "Checking…", "检查中…",
    update_up_to_date: "You are up to date", "已是最新版本",
    update_modal_title: "New version available", "发现新版本",
    update_later: "Later", "稍后",
    update_download: "Download update", "下载更新",
    update_open_releases: "All releases", "全部版本",
    update_changelog: "Release notes", "更新日志",
    update_current: "Current", "当前版本",
    update_latest: "Latest", "最新版本",
    update_failed: "Update check failed", "检查更新失败",
    update_badge: "Update", "更新",
    update_view: "View", "查看",
    update_check_on_startup: "Check for updates on startup", "启动时检查更新",
    update_whats_new: "What's new", "更新内容",
    update_section: "Desktop updates", "桌面端更新",
    version_label: "Version", "版本",
    untitled_paren: "(untitled)", "(无标题)",
    new_session_title: "New chat", "新会话",
    no_project: "(no project)", "(无项目)",
    unbound_cwd: "(no working directory)", "(未绑定工作目录)",
    click_to_preview: "Click to preview", "点击预览",
    will_bind: "Will bind", "将绑定",
    file_too_large: "File too large (>12MB)", "文件过大（>12MB）",
    decode_image_failed: "Failed to decode image", "解码图片失败",
    view_image_prompt: "Please look at the following image(s) and answer.",
        "请直接查看以下图片并回答。",
    grok_not_found: "grok executable not found. Install the Grok CLI: https://x.ai/cli — or set the path in Settings.",
        "找不到 grok 可执行文件。请先安装 Grok CLI：https://x.ai/cli ，或在设置里填写 grok 路径。",
    title_empty: "Title cannot be empty", "标题不能为空",
    not_in_app_index: "Session is not in the App index", "会话不在 App 索引中",
    invalid_session_id: "Invalid session id", "无效会话 id",
    install_ok: "Install succeeded", "安装成功",
    install_done_refresh: "Install finished. If grok is missing, reopen the terminal or click Refresh.",
        "安装脚本已完成。若找不到 grok，请重新打开终端或点「刷新状态」。",
    clipboard_busy: "Clipboard busy", "剪贴板忙碌",
    image_read_failed: "Failed to read image", "图片读取失败",
    pasted_image: "Pasted image", "已粘贴图片",
    drop_image_failed: "Drop image failed", "拖放图片失败",
    approved_tool: "Approved tool", "已批准工具",
    attach_save_failed: "Failed to save attachment", "保存附件失败",
    attach_persist_failed: "Failed to write attachment", "附件落盘失败",
    archive_failed: "Archive failed", "归档失败",
    delete_failed: "Delete failed", "删除失败",
    rename_failed: "Rename failed", "重命名失败",
    restore_failed: "Restore failed", "恢复失败",
    import_failed: "Import failed", "导入失败",
    prompt_failed: "Prompt failed", "prompt 失败",
    thought_truncated: "…(thinking truncated)", "…(思考过长已截断)",
    tools_summary: "tool calls", "个工具调用",
}

/// "Current: medium (medium)"
pub fn current_effort_line(label: &str, id: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("当前：{label}（{id}）"),
        Locale::En => format!("Current: {label} ({id})"),
    }
}

/// "12 sessions · ~/.grok/sessions"
pub fn sessions_count_line(n: usize) -> String {
    match current_locale() {
        Locale::Zh => format!("{n} 条 · ~/.grok/sessions"),
        Locale::En => format!("{n} sessions · ~/.grok/sessions"),
    }
}

pub fn version_line(v: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("版本 {v}"),
        Locale::En => format!("Version {v}"),
    }
}

pub fn will_bind_folder(name: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("✓  将绑定「{name}」"),
        Locale::En => format!("✓  Will use “{name}”"),
    }
}

pub fn click_preview(label: &str) -> String {
    format!("{} — {}", label, t().click_to_preview)
}

pub fn err_fmt(prefix_key: fn() -> &'static str, e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", prefix_key())
}

// Convenience prefixes for format errors
pub fn archive_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().archive_failed)
}
pub fn delete_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().delete_failed)
}
pub fn rename_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().rename_failed)
}
pub fn restore_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().restore_failed)
}
pub fn import_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().import_failed)
}
pub fn attach_save_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().attach_save_failed)
}
pub fn prompt_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e:#}", t().prompt_failed)
}
pub fn drop_image_failed_e(e: impl std::fmt::Display) -> String {
    format!("{}: {e}", t().drop_image_failed)
}
pub fn clipboard_busy_e(e: impl std::fmt::Display) -> String {
    format!("{}… ({e})", t().clipboard_busy)
}
pub fn image_read_failed_e(fmt: &str) -> String {
    format!("{} ({fmt})", t().image_read_failed)
}
pub fn pasted_image_s(label: &str) -> String {
    format!("{} {label}", t().pasted_image)
}
pub fn approved_tool_s(opt: &str) -> String {
    format!("{} · {opt}", t().approved_tool)
}
pub fn max_attachments_s(n: usize) -> String {
    max_attachments(n)
}
pub fn attached_progress_s(cur: usize, max: usize, n: usize) -> String {
    attached_progress(cur, max, n)
}
pub fn image_n_path_s(n: usize, path: &str) -> String {
    image_n_path(n, path)
}
pub fn tools_summary_line(n: usize, titles: &str) -> String {
    match current_locale() {
        Locale::Zh => format!("⚙ {n} 个工具调用: {titles}"),
        Locale::En => format!("⚙ {n} tool calls: {titles}"),
    }
}
