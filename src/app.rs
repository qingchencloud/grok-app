use crate::acp::parse::build_history_bootstrap;
use crate::acp::{AcpClient, AgentEvent, ChatImage, PermissionOption, TimelineItem};
use crate::attachments::{self, PendingImage};
use crate::config::{
    auth_credentials_changed, auth_file_stamp, effort_label, is_authentication_required_error,
    is_cli_authenticated, normalize_effort, resolve_grok_binary, AgentMode, AppConfig,
    AuthFileStamp, MODELS,
};
use crate::desktop_notify;
use crate::desktop_tray::{AppTray, TrayAction};
use crate::image_generation::{
    spawn_generate, ImageGenerationEvent, ImageGenerationRequest, API_KEY_URL,
};
use crate::local::install::{install_cli, probe_status_fast, InstallProgress};
use crate::local::{
    archive_session, delete_from_app, group_sessions_by_project, import_cli_session,
    list_active_sessions, list_archived_sessions, list_cli_import_candidates,
    load_session_timeline, normalize_project_key, rename_in_index, rename_session, restore_session,
    sync_record_from_disk, touch_session, LocalSession,
};
use crate::session_store::{PendingPermission, SessionStore, TurnPhase};
use crate::stream::SmoothStream;
use crate::ui::chat_view;
use crate::ui::icons::{self, IconKind};
use crate::ui::settings::{draw_settings, SettingsState, SettingsTab};
use crate::ui::slash::{self, SlashAction};
use crate::ui::theme;
use crate::ui::widgets::{
    self, context_meter, ghost_button, hairline, nav_row, primary_button, project_row, quiet_link,
    search_field, session_row, soft_action, status_dot, status_pill, tree_section_head,
    SessionActivity,
};
use crate::update::{self, UpdateCheckResult, UpdateUiState};
use eframe::egui;
use egui::{
    Align, Color32, Frame, Key, Layout, Margin, Modifiers, RichText, ScrollArea, Stroke, TextEdit,
    Ui,
};
use egui_commonmark::CommonMarkCache;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct GrokApp {
    config: AppConfig,
    settings: SettingsState,

    show_logs: bool,
    sidebar_open: bool,
    /// Scroll sidebar session list into view / expand section
    focus_sessions: bool,

    input: String,
    /// Images waiting to be sent with the next message (Ctrl+V / 附件 / 拖放).
    pending_images: Vec<PendingImage>,
    /// Cached egui textures for pending image thumbs (id → handle).
    thumb_textures: std::collections::HashMap<String, egui::TextureHandle>,
    timeline: Vec<TimelineItem>,
    logs: Vec<String>,
    /// App-index active sessions only (not CLI dump, not archived).
    local_sessions: Vec<LocalSession>,
    /// Cached archived list for the archive panel.
    archived_sessions: Vec<LocalSession>,
    /// CLI import picker candidates.
    import_candidates: Vec<LocalSession>,
    show_archive_panel: bool,
    show_import_panel: bool,
    /// Filter for main session list.
    session_filter: String,
    /// Filter inside CLI import panel.
    import_filter: String,
    /// Project keys (normalized cwd) that the user collapsed in the sidebar.
    collapsed_projects: HashSet<String>,
    /// Rename dialog: (session, draft title).
    rename_draft: Option<(LocalSession, String)>,
    /// Chat scroll is not at bottom → show floating jump button.
    chat_away_from_bottom: bool,
    /// Scroll the timeline so this item id is visible (one-shot, cleared after paint).
    scroll_to_item_id: Option<String>,
    /// Last user-message id we jumped to (for prev/next chain).
    focused_user_msg_id: Option<String>,
    /// After Ctrl+V: retry clipboard image for a few frames (Windows often locks
    /// the clipboard on the exact key frame, so one-shot reads fail).
    paste_probe_frames: u8,
    /// Snapshot of Event::Paste texts from the trigger frame (for text fallback).
    paste_probe_texts: Vec<String>,
    /// Edge-detect Ctrl+V (key_pressed is unreliable with focused TextEdit).
    prev_ctrl_v_down: bool,
    /// Cached: clipboard currently advertises an image format (cheap poll).
    clipboard_image_ready: bool,
    /// Full-size image preview (composer thumb or message attachment).
    image_preview: Option<ChatImage>,
    /// Textures for images already sent in the timeline (message id+img id).
    message_textures: std::collections::HashMap<String, egui::TextureHandle>,

    status: String,
    agent_label: String,
    /// Single source of truth for connection + turn runtime.
    store: SessionStore,
    /// Agent child PID (process management display).
    agent_pid: Option<u32>,
    /// Last known context usage (tokens).
    context_used: Option<u64>,
    context_max: Option<u64>,
    context_note: Option<String>,

    error_banner: Option<String>,
    scroll_to_bottom: bool,
    input_focus_request: bool,

    rt: tokio::runtime::Runtime,
    client: Arc<Mutex<Option<Arc<AcpClient>>>>,
    /// Invalidates stale asynchronous connect attempts. A superseded client is
    /// shut down before it can overwrite the active slot or emit Connected.
    connect_generation: Arc<AtomicU64>,
    event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,

    install_rx: Option<std_mpsc::Receiver<InstallProgress>>,
    /// First-run/update gate: Grok CLI must exist and be authenticated.
    onboarding_open: bool,
    login_started: bool,
    login_rx: Option<std_mpsc::Receiver<Result<(), String>>>,
    /// Credential metadata captured immediately before `grok login`.
    login_auth_stamp: Option<AuthFileStamp>,
    /// Credential metadata inherited by the currently running agent.
    agent_auth_stamp: Option<AuthFileStamp>,
    /// Runtime ACP rejection overrides a merely-present auth.json.
    runtime_auth_rejected: bool,
    last_readiness_probe: Option<std::time::Instant>,

    /// Grok Imagine (xAI Images API) session-only state.
    image_generation_open: bool,
    image_api_key: String,
    image_prompt: String,
    image_aspect_ratio: String,
    image_resolution: String,
    image_generating: bool,
    image_generation_rx: Option<std_mpsc::Receiver<ImageGenerationEvent>>,

    /// Product share card shown from the sidebar or Settings → About.
    share_open: bool,
    share_copied: bool,

    md_cache: CommonMarkCache,

    /// Smooth reveal for live assistant / thought text (view-only; not business state).
    smooth_assistant: SmoothStream,
    smooth_thought: SmoothStream,

    /// Connect agent after a few UI frames (more stable startup).
    pending_connect: bool,
    frame_count: u32,

    /// New-chat dialog: bind working directory (default = config.cwd).
    new_chat_draft: Option<NewChatDraft>,
    /// Background repaint pump cancel (stream keep-alive).
    stream_pump: Option<tokio::sync::oneshot::Sender<()>>,
    /// After failed session/load: inject history on next prompt.
    needs_history_bootstrap: bool,
    /// While true, ignore streamed session/update history (load already
    /// painted the timeline from disk). Prevents duplicate “轮回” messages.
    suppress_stream_updates: bool,
    /// Throttle disk session rescans.
    last_session_scan: Option<std::time::Instant>,
    /// Show full timeline (vs last N items).
    show_all_history: bool,
    /// Slash palette selection index.
    slash_selected: usize,
    /// GitHub Releases update check.
    update: UpdateUiState,
    update_rx: Option<std_mpsc::Receiver<UpdateCheckResult>>,
    update_tx: std_mpsc::Sender<UpdateCheckResult>,
    /// Startup update check deferred a few frames.
    pending_update_check: bool,
    /// System tray (optional).
    tray: Option<AppTray>,
    /// Window currently hidden (to tray).
    window_hidden: bool,
    /// Next close should quit (from tray Quit), not hide.
    quit_requested: bool,
}

struct NewChatDraft {
    cwd: String,
}

impl GrokApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        crate::i18n::set_locale(config.locale());
        // Must load CJK fonts BEFORE first frame — default egui fonts have no 中文.
        crate::ui::fonts::install(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, config.dark_mode);

        if (config.font_scale - 1.0).abs() > 0.01 {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.text_styles.iter_mut().for_each(|(_, font)| {
                font.size *= config.font_scale;
            });
            cc.egui_ctx.set_style(style);
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("grok-app-worker")
            .build()
            .expect("tokio runtime");

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // App index only — never auto-dump ~/.grok/sessions into the sidebar
        let local_sessions = std::panic::catch_unwind(list_active_sessions).unwrap_or_default();
        let settings = SettingsState::new(&config);

        // Sync the CLI's startup mode when it explicitly selects one of the
        // three supported desktop modes.
        let mut config = config;
        if settings.cli_toml.loaded {
            if settings.cli_toml.yolo
                || settings.cli_toml.permission_mode == "always-approve"
                || settings.cli_toml.permission_mode == "bypassPermissions"
            {
                config.set_agent_mode(AgentMode::AlwaysApprove);
            } else if settings.cli_toml.permission_mode == "plan" {
                config.set_agent_mode(AgentMode::Plan);
            }
            if !settings.cli_toml.default_model.is_empty() && config.model == "grok-4.5" {
                // keep app model; user can still override
            }
        }

        let mut app = Self {
            settings,
            config,
            show_logs: false,
            sidebar_open: true,
            focus_sessions: false,
            input: String::new(),
            pending_images: Vec::new(),
            thumb_textures: std::collections::HashMap::new(),
            timeline: Vec::new(),
            logs: Vec::new(),
            local_sessions,
            archived_sessions: Vec::new(),
            import_candidates: Vec::new(),
            show_archive_panel: false,
            show_import_panel: false,
            session_filter: String::new(),
            import_filter: String::new(),
            collapsed_projects: HashSet::new(),
            rename_draft: None,
            chat_away_from_bottom: false,
            scroll_to_item_id: None,
            focused_user_msg_id: None,
            paste_probe_frames: 0,
            paste_probe_texts: Vec::new(),
            prev_ctrl_v_down: false,
            clipboard_image_ready: false,
            image_preview: None,
            message_textures: std::collections::HashMap::new(),
            status: crate::i18n::t().not_connected.into(),
            agent_label: String::new(),
            store: SessionStore::new(),
            agent_pid: None,
            context_used: None,
            context_max: None,
            context_note: None,
            error_banner: None,
            scroll_to_bottom: false,
            input_focus_request: true,
            rt,
            client: Arc::new(Mutex::new(None)),
            connect_generation: Arc::new(AtomicU64::new(0)),
            event_rx,
            event_tx,
            install_rx: None,
            onboarding_open: false,
            login_started: false,
            login_rx: None,
            login_auth_stamp: None,
            agent_auth_stamp: None,
            runtime_auth_rejected: false,
            last_readiness_probe: None,
            image_generation_open: false,
            image_api_key: std::env::var("XAI_API_KEY").unwrap_or_default(),
            image_prompt: String::new(),
            image_aspect_ratio: "auto".into(),
            image_resolution: "1k".into(),
            image_generating: false,
            image_generation_rx: None,
            share_open: false,
            share_copied: false,
            md_cache: CommonMarkCache::default(),
            smooth_assistant: SmoothStream::default(),
            smooth_thought: SmoothStream::default(),
            pending_connect: false,
            frame_count: 0,
            new_chat_draft: None,
            stream_pump: None,
            needs_history_bootstrap: false,
            suppress_stream_updates: false,
            last_session_scan: None,
            show_all_history: false,
            slash_selected: 0,
            update: UpdateUiState::new(),
            update_rx: None,
            update_tx: {
                let (tx, _rx) = update::channel();
                tx
            },
            pending_update_check: false,
            tray: None,
            window_hidden: false,
            quit_requested: false,
        };

        let (utx, urx) = update::channel();
        app.update_tx = utx;
        app.update_rx = Some(urx);

        // Seed context window from ~/.grok/models_cache.json (not a hard-coded default).
        app.refresh_context_from_catalog();

        // Defer agent connect — spawning grok during first frames can race with
        // window creation and look like a crash on some machines.
        let cli_ok = resolve_grok_binary(&app.config.grok_path).is_ok();
        let auth_ok = is_cli_authenticated();
        app.pending_connect = cli_ok && auth_ok && app.config.auto_connect;
        app.onboarding_open = !cli_ok || !auth_ok;
        app.pending_update_check = app.config.check_updates_on_startup;
        if !cli_ok {
            app.settings.tab = SettingsTab::Cli;
        }

        if !auth_ok {
            app.logs.push(crate::i18n::t().auth_missing_hint.into());
        }

        app.frame_count = 0;
        // The startup surface is an explicit unsaved draft. Its first send must
        // always create a new session, never reuse a stale ACP binding.
        app.new_chat_draft = Some(NewChatDraft {
            cwd: app.config.cwd.clone(),
        });
        app
    }

    fn start_update_check(&mut self, ctx: &egui::Context) {
        if self.update.checking {
            return;
        }
        self.update.checking = true;
        self.update.error = None;
        update::spawn_check(self.update_tx.clone(), self.update.current.clone());
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn poll_update_check(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.update_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(res) => {
                let dismissed = self.config.update_dismissed_tag.clone();
                self.update.apply_result(res, &dismissed);
                ctx.request_repaint();
            }
            Err(std_mpsc::TryRecvError::Empty) => {}
            Err(std_mpsc::TryRecvError::Disconnected) => {}
        }
    }

    fn dismiss_update_modal(&mut self) {
        self.update.modal_open = false;
        if let Some(tag) = self.update.latest.as_ref().map(|r| r.tag.clone()) {
            self.config.update_dismissed_tag = tag;
            let _ = self.config.save();
        }
    }

    fn open_update_download(&mut self) {
        let url = self
            .update
            .selected_release()
            .and_then(|r| r.setup_url.clone().or_else(|| r.portable_url.clone()))
            .or_else(|| self.update.selected_release().map(|r| r.html_url.clone()))
            .unwrap_or_else(|| update::LATEST_URL.to_string());
        update::open_url(&url);
    }

    fn refresh_sessions(&mut self) {
        self.local_sessions = list_active_sessions();
        if self.show_archive_panel {
            self.archived_sessions = list_archived_sessions();
        }
        if self.show_import_panel {
            self.import_candidates = list_cli_import_candidates(80);
        }
        self.last_session_scan = Some(std::time::Instant::now());
    }

    /// Optimistic sidebar + durable App index update.
    fn touch_session_list(&mut self, user_text: &str) {
        let title_hint: String = user_text
            .chars()
            .take(36)
            .collect::<String>()
            .replace('\n', " ");
        if let Some(sid) = self.store.session_id_owned() {
            let _ = touch_session(
                &sid,
                Some(title_hint.as_str()).filter(|s| !s.is_empty()),
                Some(self.config.cwd.as_str()),
                Some(self.config.model.as_str()),
                true,
            );
            // Refresh in-memory list from index (keeps sort/order correct)
            self.local_sessions = list_active_sessions();
        }
    }

    /// Register / refresh a session id into the App index (isolation boundary).
    fn register_app_session(&mut self, session_id: &str) {
        let title_hint = self.timeline.iter().rev().find_map(|item| match item {
            TimelineItem::UserMessage { text, .. } => {
                let title = text.chars().take(36).collect::<String>().replace('\n', " ");
                (!title.trim().is_empty()).then_some(title)
            }
            _ => None,
        });
        let _ = touch_session(
            session_id,
            title_hint.as_deref(),
            Some(self.config.cwd.as_str()),
            Some(self.config.model.as_str()),
            false,
        );
        let _ = sync_record_from_disk(session_id);
        self.local_sessions = list_active_sessions();
    }

    /// End turn only if `turn_gen` still owns the UI (ignores stale RPC completions).
    fn finish_turn_if_gen(&mut self, turn_gen: u64, status: impl Into<String>) -> bool {
        if !self.store.end_turn_if_gen(turn_gen) {
            return false;
        }
        self.stop_stream_pump();
        self.smooth_assistant.finish();
        self.smooth_thought.finish();
        self.finalize_open_streams();
        self.mark_running_tools_terminal("completed");
        self.status = status.into();
        true
    }

    /// ACP: only the agent ends a turn (session/prompt response). Host must NOT
    /// invent early ends or spam session/cancel — that aborts live tool loops
    /// ("通道中断了"). We only soft-heal display chips; never cancel here.
    fn heal_tool_display_only(&mut self) {
        if !self.store.busy() {
            return;
        }
        // Clear live-tool chip when no running tool rows remain (display only).
        let any_running = self.timeline.iter().any(|i| match i {
            TimelineItem::Tool { status, .. } => crate::acp::parse::tool_status_is_running(status),
            _ => false,
        });
        if !any_running {
            self.store.clear_live_tool();
        }
    }

    fn mark_running_tools_terminal(&mut self, terminal: &str) {
        for item in &mut self.timeline {
            if let TimelineItem::Tool { status, .. } = item {
                let s = status.to_ascii_lowercase();
                if s == "pending" || s == "in_progress" || s == "running" {
                    *status = terminal.into();
                }
            }
        }
    }

    /// If store is idle, never keep streaming flags / live tool chip.
    fn reconcile_idle_display(&mut self) {
        if self.prompt_active() {
            return;
        }
        if self.store.live_tool().is_some() {
            self.store.clear_live_tool();
        }
        let mut dirty = false;
        for item in &mut self.timeline {
            if let TimelineItem::AssistantMessage { streaming, .. } = item {
                if *streaming {
                    *streaming = false;
                    dirty = true;
                }
            }
            if let TimelineItem::Tool { status, .. } = item {
                let s = status.to_ascii_lowercase();
                if s == "pending" || s == "in_progress" || s == "running" {
                    *status = "completed".into();
                    dirty = true;
                }
            }
        }
        if dirty {
            self.store.clear_stream_cursors();
            self.smooth_assistant.finish();
            self.smooth_thought.finish();
        }
    }

    /// Source of truth for whether Stop must remain available.
    ///
    /// The ACP request can still be active for a short time after a stale UI
    /// event cleared `SessionStore`; never hide Stop or allow another send then.
    fn prompt_active(&self) -> bool {
        self.store.busy()
            || self
                .client
                .lock()
                .as_ref()
                .map(|client| client.is_prompt_inflight())
                .unwrap_or(false)
    }

    fn collect_history_turns(&self) -> Vec<(bool, String)> {
        let mut out = Vec::new();
        for item in &self.timeline {
            match item {
                TimelineItem::UserMessage { text, .. } if !text.is_empty() => {
                    out.push((true, text.clone()));
                }
                TimelineItem::AssistantMessage { text, .. } if !text.is_empty() => {
                    out.push((false, text.clone()));
                }
                _ => {}
            }
        }
        out
    }

    fn start_cli_install(&mut self) {
        if self.settings.installing {
            return;
        }
        self.onboarding_open = true;
        self.settings.installing = true;
        self.settings.install_logs.clear();
        self.settings
            .install_logs
            .push(crate::i18n::t().install_start.into());
        self.settings.tab = SettingsTab::Cli;
        let (tx, rx) = std_mpsc::channel();
        self.install_rx = Some(rx);
        install_cli(tx);
    }

    fn poll_install(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(rx) = self.install_rx.as_ref() {
            while let Ok(ev) = rx.try_recv() {
                events.push(ev);
            }
        }
        if events.is_empty() {
            return;
        }
        let mut reconnect = false;
        let mut done = false;
        for ev in events {
            match ev {
                InstallProgress::Started => {
                    self.settings
                        .install_logs
                        .push(crate::i18n::t().install_started.into());
                }
                InstallProgress::Log(line) => {
                    self.settings.install_logs.push(line);
                    if self.settings.install_logs.len() > 300 {
                        let n = self.settings.install_logs.len() - 200;
                        self.settings.install_logs.drain(0..n);
                    }
                }
                InstallProgress::Finished { ok, message } => {
                    self.settings.installing = false;
                    self.settings.install_logs.push(message.clone());
                    self.settings.message = Some(message);
                    self.settings.refresh_cli();
                    reconnect = ok;
                    done = true;
                }
            }
        }
        if done {
            self.install_rx = None;
        }
        if reconnect {
            if is_cli_authenticated() {
                self.onboarding_open = false;
                self.connect_agent(ctx);
            } else {
                self.onboarding_open = true;
                self.login_started = false;
            }
        }
        ctx.request_repaint();
    }

    fn readiness(&self) -> (bool, bool) {
        (
            resolve_grok_binary(&self.config.grok_path).is_ok(),
            is_cli_authenticated() && !self.runtime_auth_rejected,
        )
    }

    fn poll_readiness(&mut self, ctx: &egui::Context) {
        let interval = if self.onboarding_open || self.login_started {
            std::time::Duration::from_millis(750)
        } else {
            std::time::Duration::from_secs(5)
        };
        let due = self
            .last_readiness_probe
            .map(|last| last.elapsed() >= interval)
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_readiness_probe = Some(std::time::Instant::now());
        self.settings.cli_status = probe_status_fast(&self.config.grok_path);
        let credentials_present = self.settings.cli_status.authenticated;
        let credentials_updated = self.login_started
            && credentials_present
            && auth_credentials_changed(self.login_auth_stamp.as_ref(), auth_file_stamp().as_ref());
        if credentials_updated {
            self.complete_grok_login(ctx);
            return;
        }
        if self.runtime_auth_rejected {
            self.settings.cli_status.authenticated = false;
        }
        let installed = self.settings.cli_status.installed;
        let authenticated = self.settings.cli_status.authenticated;
        if installed && authenticated {
            self.onboarding_open = false;
            self.login_started = false;
            self.status = crate::i18n::t().onboarding_ready.into();
            if !self.store.is_connected() && !self.store.is_connecting() {
                self.pending_connect = true;
            }
        } else if !self.settings.open {
            self.onboarding_open = true;
        }
        ctx.request_repaint();
    }

    fn complete_grok_login(&mut self, ctx: &egui::Context) {
        self.login_started = false;
        self.login_rx = None;
        self.login_auth_stamp = None;
        self.runtime_auth_rejected = false;
        self.settings.cli_status = probe_status_fast(&self.config.grok_path);
        self.onboarding_open = false;
        self.status = crate::i18n::t().connecting_ellipsis.into();
        // Reconnect even when the old agent reports Connected: it was spawned
        // before login and does not reload auth.json at runtime.
        self.pending_connect = true;
        self.last_readiness_probe = Some(std::time::Instant::now());
        self.logs
            .push("登录凭据已更新，正在重启 Agent 以加载新认证".into());
        ctx.request_repaint();
    }

    fn poll_login(&mut self, ctx: &egui::Context) {
        let result = match self.login_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(std_mpsc::TryRecvError::Disconnected)) => {
                Some(Err("grok login process channel disconnected".into()))
            }
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.login_rx = None;

        if result.is_ok() && is_cli_authenticated() {
            self.complete_grok_login(ctx);
            return;
        }

        self.login_started = false;
        self.login_auth_stamp = None;
        self.settings.cli_status = probe_status_fast(&self.config.grok_path);
        if self.runtime_auth_rejected {
            self.settings.cli_status.authenticated = false;
        }
        self.onboarding_open = true;
        if let Err(message) = result {
            self.error_banner = Some(message.clone());
            self.logs.push(format!("登录进程失败: {message}"));
        }
        ctx.request_repaint();
    }

    fn start_image_generation(&mut self) {
        if self.image_generating {
            return;
        }
        if self.pending_images.len() >= attachments::MAX_ATTACHMENTS {
            self.error_banner = Some(crate::i18n::max_attachments(attachments::MAX_ATTACHMENTS));
            return;
        }
        if self.image_api_key.trim().is_empty() {
            self.error_banner = Some(crate::i18n::t().image_api_key_missing.into());
            return;
        }
        if self.image_prompt.trim().is_empty() {
            self.error_banner = Some(crate::i18n::t().image_prompt_hint.into());
            return;
        }

        let request = ImageGenerationRequest {
            api_key: self.image_api_key.trim().to_string(),
            prompt: self.image_prompt.trim().to_string(),
            aspect_ratio: self.image_aspect_ratio.clone(),
            resolution: self.image_resolution.clone(),
        };
        let (tx, rx) = std_mpsc::channel();
        self.image_generation_rx = Some(rx);
        self.image_generating = true;
        self.status = crate::i18n::t().image_generating.into();
        self.error_banner = None;
        spawn_generate(request, tx);
    }

    fn poll_image_generation(&mut self, ctx: &egui::Context) {
        let result = self
            .image_generation_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        let Some(ImageGenerationEvent::Finished(result)) = result else {
            return;
        };
        self.image_generation_rx = None;
        self.image_generating = false;
        match result {
            Ok(mut image) => {
                let next = (self.pending_images.len() + 1) as u32;
                if let Err(error) = image.ensure_on_disk(next) {
                    self.error_banner = Some(format!(
                        "{}: {error:#}",
                        crate::i18n::t().image_generation_failed
                    ));
                } else {
                    self.image_preview = Some(image.to_chat_image());
                    self.pending_images.push(image);
                    self.image_prompt.clear();
                    self.image_generation_open = false;
                    self.status = crate::i18n::t().image_generated.into();
                }
            }
            Err(error) => {
                self.error_banner = Some(format!(
                    "{}: {error}",
                    crate::i18n::t().image_generation_failed
                ));
                self.status = crate::i18n::t().error_status.into();
            }
        }
        ctx.request_repaint();
    }

    fn handle_settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings.open {
            return;
        }
        // Keep drafts in sync when opening
        let sessions = self.local_sessions.clone();
        let actions = draw_settings(ctx, &mut self.settings, &sessions, &self.update);

        if actions.start_install {
            self.start_cli_install();
        }
        if actions.open_login {
            self.run_grok_login();
        }
        if actions.open_sessions_focus {
            self.focus_sessions = true;
            self.settings.open = false;
        }
        if actions.apply_theme {
            self.config.dark_mode = self.settings.dark_mode;
            theme::apply(ctx, self.config.dark_mode);
            let _ = self.config.save();
            self.settings.message = Some(if self.config.dark_mode {
                crate::i18n::t().status_switched_dark.into()
            } else {
                crate::i18n::t().status_switched_light.into()
            });
        }
        if actions.check_updates {
            self.start_update_check(ctx);
        }
        if actions.open_update_modal {
            self.update.modal_open = true;
            if let Some(tag) = self.update.latest.as_ref().map(|r| r.tag.clone()) {
                self.update.selected_tag = Some(tag);
            }
        }
        if actions.open_share {
            self.settings.open = false;
            self.share_open = true;
            self.share_copied = false;
        }
        if actions.save_config {
            self.apply_settings_to_config();
            if let Err(e) = self.config.save() {
                self.error_banner = Some(crate::i18n::save_failed(&e));
            } else {
                theme::apply(ctx, self.config.dark_mode);
                self.apply_font_scale(ctx);
                self.settings.message = Some(crate::i18n::t().settings_saved.into());
                if self.config.show_tray {
                    self.ensure_tray(ctx);
                } else {
                    self.tray = None;
                }
                if actions.reconnect {
                    self.connect_agent(ctx);
                }
            }
        }
        if actions.save_agent_and_reconnect {
            self.apply_settings_to_config();
            theme::apply(ctx, self.config.dark_mode);
            self.apply_font_scale(ctx);
            if let Err(e) = self.config.save() {
                self.error_banner = Some(crate::i18n::save_failed(&e));
            } else {
                self.settings.open = false;
                self.settings.message = Some(crate::i18n::t().agent_settings_saved.into());
                self.connect_agent(ctx);
            }
        }
    }

    fn apply_font_scale(&self, ctx: &egui::Context) {
        // Re-install base fonts then scale — fonts::install resets sizes.
        crate::ui::fonts::install(ctx);
        let scale = self.config.font_scale.clamp(0.85, 1.35);
        if (scale - 1.0).abs() > 0.01 {
            let mut style = (*ctx.style()).clone();
            style.text_styles.iter_mut().for_each(|(_, font)| {
                font.size *= scale;
            });
            ctx.set_style(style);
        }
    }

    fn apply_settings_to_config(&mut self) {
        let s = &self.settings;
        self.config.grok_path = s.grok_path.trim().to_string();
        self.config.cwd = s.cwd.trim().to_string();
        self.config.model = s.model.trim().to_string();
        self.config.effort = normalize_effort(&s.effort).to_string();
        self.config
            .set_agent_mode(AgentMode::from_id(&s.permission_mode));
        self.config.dark_mode = s.dark_mode;
        if s.ui_locale == "system" {
            self.config.ui_locale_mode = "system".into();
        } else {
            self.config.ui_locale_mode = "manual".into();
            self.config.ui_locale = crate::i18n::Locale::from_str(&s.ui_locale)
                .as_str()
                .to_string();
        }
        crate::i18n::set_locale(self.config.locale());
        self.config.font_scale = s.font_scale.clamp(0.85, 1.35);
        self.config.auto_connect = s.auto_connect;
        self.config.smooth_stream = s.smooth_stream;
        self.config.show_thoughts = s.show_thoughts;
        self.config.expand_tools = s.expand_tools;
        self.config.enter_to_send = s.enter_to_send;
        self.config.set_extra_args_line(&s.extra_args);
        self.config.user_display_name = AppConfig::sanitize_display_name(&s.user_display_name);
        self.config.user_avatar_path = s.user_avatar_path.trim().to_string();
        self.config.check_updates_on_startup = s.check_updates_on_startup;
        self.config.show_tray = s.show_tray;
        self.config.close_to_tray = s.close_to_tray;
        self.config.notify_on_turn_complete = s.notify_on_turn_complete;
        self.config.notify_only_when_unfocused = s.notify_only_when_unfocused;
        // Keep settings fields in sync with sanitized values
        self.settings.user_display_name = self.config.user_display_name.clone();
        self.settings.user_avatar_path = self.config.user_avatar_path.clone();
    }

    fn ensure_tray(&mut self, ctx: &egui::Context) {
        if !self.config.show_tray {
            self.tray = None;
            return;
        }
        if self.tray.is_some() {
            return;
        }
        let tip = format!("Grok  v{}", env!("CARGO_PKG_VERSION"));
        match AppTray::try_new(&tip, ctx.clone()) {
            Ok(t) => {
                self.tray = Some(t);
                self.logs.push(crate::i18n::t().tray_enable.to_string());
            }
            Err(e) => {
                tracing::warn!("tray init failed: {e:#}");
                self.logs.push(format!("tray: {e:#}"));
            }
        }
    }

    fn show_main_window(&mut self, ctx: &egui::Context) {
        self.window_hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        crate::win_chrome::apply_titlebar_theme(self.config.dark_mode);
    }

    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        self.window_hidden = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let actions: Vec<TrayAction> = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for a in actions {
            match a {
                TrayAction::Show => self.show_main_window(ctx),
                TrayAction::Hide => {
                    self.ensure_tray(ctx);
                    self.hide_to_tray(ctx);
                }
                TrayAction::Toggle => {
                    if self.window_hidden {
                        self.show_main_window(ctx);
                    } else {
                        self.hide_to_tray(ctx);
                    }
                }
                TrayAction::Quit => {
                    self.quit_requested = true;
                    self.window_hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn handle_close_to_tray(&mut self, ctx: &egui::Context) {
        let close = ctx.input(|i| i.viewport().close_requested());
        if !close {
            return;
        }
        if self.quit_requested {
            return;
        }
        if self.config.close_to_tray && self.config.show_tray {
            self.ensure_tray(ctx);
            if self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide_to_tray(ctx);
            }
        }
    }

    /// Desktop toast when a turn ends (respects settings + focus).
    fn maybe_notify_turn_complete(&self, ctx: &egui::Context, cancelled: bool) {
        if !self.config.notify_on_turn_complete {
            return;
        }
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        if self.config.notify_only_when_unfocused && focused && !self.window_hidden {
            return;
        }
        let s = crate::i18n::t();
        if cancelled {
            desktop_notify::notify_turn_done(s.notify_turn_cancelled, s.notify_turn_body);
        } else {
            desktop_notify::notify_turn_done(s.notify_turn_title, s.notify_turn_body);
        }
    }

    fn open_local_session(&mut self, sess: &LocalSession, ctx: &egui::Context) {
        if self.prompt_active() {
            self.error_banner = Some(crate::i18n::t().err_busy.into());
            return;
        }
        self.new_chat_draft = None;
        // Preview timeline from disk immediately (last N items of full parse).
        // session/load will re-broadcast the same history as session/update —
        // suppress those so we do not double-render ("轮回" duplicates).
        let timeline = load_session_timeline(&sess.path, 200);
        self.timeline = timeline;
        self.show_all_history = false;
        self.smooth_assistant.clear();
        self.smooth_thought.clear();
        self.store.on_switch_session_view(sess.id.clone());
        self.store.clear_stream_cursors();
        // Only clear live tool row when viewing a different session than the busy one
        if self.store.busy_session_id() != Some(sess.id.as_str()) {
            // keep sticky busy; tool chip is for active turn — hide on foreign view
            // (sidebar still uses busy_session_id)
        }
        self.scroll_to_bottom = true;
        self.scroll_to_item_id = None;
        self.focused_user_msg_id = None;
        self.input.clear();
        self.slash_selected = 0;
        self.pending_images.clear();
        self.thumb_textures.clear();
        self.message_textures.clear();
        self.needs_history_bootstrap = false;
        self.suppress_stream_updates = true;
        if !sess.cwd.is_empty() {
            self.config.cwd = sess.cwd.clone();
        }

        // Expand this session's project group in the sidebar
        let pkey = normalize_project_key(&sess.cwd);
        if !pkey.is_empty() {
            self.collapsed_projects.remove(&pkey);
            self.collapsed_projects.insert(format!("__exp:{pkey}"));
        }

        if !sess.cwd.is_empty() {
            self.config.cwd = sess.cwd.clone();
            let _ = self.config.save();
        }
        if !sess.model.is_empty() {
            self.config.model = sess.model.clone();
        }

        let sid = sess.id.clone();
        let cwd = sess.cwd.clone();
        self.status = crate::i18n::session_label(&short_id(&sid));

        let client = self.client.lock().clone();
        if let Some(client) = client {
            let event_tx = self.event_tx.clone();
            let repaint = ctx.clone();
            self.rt.spawn(async move {
                match client.load_session(&sid, &cwd).await {
                    Ok(_) => {
                        let _ = event_tx.send(AgentEvent::Log {
                            message: format!("SESSION_LOAD_DONE:{sid}"),
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AgentEvent::Log {
                            message: format!(
                                "BOOTSTRAP_NEEDED:{sid}:session/load 失败（已显示本地历史）: {e:#}"
                            ),
                        });
                    }
                }
                repaint.request_repaint();
            });
        } else {
            // Offline view: local history only
            self.suppress_stream_updates = false;
            self.connect_agent(ctx);
        }
    }

    // ---------- Agent control ----------

    fn connect_agent(&mut self, ctx: &egui::Context) {
        if self.store.is_connecting() {
            return;
        }
        let (installed, authenticated) = self.readiness();
        if !installed || !authenticated {
            self.pending_connect = false;
            self.onboarding_open = true;
            self.settings.cli_status = probe_status_fast(&self.config.grok_path);
            self.store.disconnect();
            self.status = crate::i18n::t().onboarding_required.into();
            ctx.request_repaint();
            return;
        }
        self.store.begin_connect();
        self.agent_pid = None;
        self.status = crate::i18n::t().connecting_ellipsis.into();
        self.error_banner = None;
        self.agent_auth_stamp = auth_file_stamp();

        let generation = self.connect_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let old_client = self.client.lock().take();
        let config = self.config.clone();
        let client_slot = self.client.clone();
        let connect_generation = self.connect_generation.clone();
        let event_tx = self.event_tx.clone();
        let repaint = ctx.clone();

        self.rt.spawn(async move {
            // A reconnect owns exactly one child process. Finish shutting down
            // the previous process before spawning its replacement.
            if let Some(old) = old_client {
                old.shutdown().await;
            }
            if connect_generation.load(Ordering::SeqCst) != generation {
                return;
            }

            match AcpClient::start(&config, event_tx.clone()).await {
                Ok(client) => {
                    if connect_generation.load(Ordering::SeqCst) != generation {
                        client.shutdown().await;
                        return;
                    }
                    let pid = client.child_pid();
                    let (agent_name, agent_version) = client.agent_info();
                    *client_slot.lock() = Some(Arc::new(client));
                    let _ = event_tx.send(AgentEvent::Connected {
                        agent_name,
                        agent_version,
                    });
                    if let Some(pid) = pid {
                        let _ = event_tx.send(AgentEvent::Log {
                            message: format!("AGENT_PID:{pid}"),
                        });
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(AgentEvent::Error {
                        message: crate::i18n::connect_failed(e),
                        turn_gen: None,
                    });
                }
            }
            repaint.request_repaint();
        });
    }

    fn disconnect_agent(&mut self) {
        self.connect_generation.fetch_add(1, Ordering::SeqCst);
        self.stop_stream_pump();
        if let Some(c) = self.client.lock().take() {
            self.rt.spawn(async move {
                c.shutdown().await;
            });
        }
        self.agent_pid = None;
        self.store.disconnect();
        self.status = crate::i18n::t().status_disconnected.into();
    }

    /// Rough local context estimate from timeline (fallback when CLI has no usage).
    fn estimate_context_tokens(&self) -> u64 {
        let mut chars = 0usize;
        for item in &self.timeline {
            match item {
                TimelineItem::UserMessage { text, .. }
                | TimelineItem::AssistantMessage { text, .. }
                | TimelineItem::Thought { text, .. }
                | TimelineItem::Status { text, .. } => chars += text.chars().count(),
                TimelineItem::Tool { title, detail, .. } => {
                    chars += title.chars().count() + detail.chars().count();
                }
                TimelineItem::Plan { entries, .. } => {
                    for e in entries {
                        chars += e.content.chars().count();
                    }
                }
            }
        }
        // ~4 chars / token heuristic
        (chars as u64 / 4).max(if self.timeline.is_empty() { 0 } else { 1 })
    }

    /// Window size: live usage max → models_cache.json for current model → unknown.
    fn resolved_context_max(&self) -> Option<u64> {
        if let Some(m) = self.context_max {
            if m > 0 {
                return Some(m);
            }
        }
        crate::models_cache::context_window_for(&self.config.model)
    }

    fn context_label(&self) -> String {
        let used = self
            .context_used
            .unwrap_or_else(|| self.estimate_context_tokens());
        match self.resolved_context_max() {
            Some(max) => format_tokens(used, max),
            None => format!("{}·?", format_tokens_one(used)),
        }
    }

    fn refresh_context_from_catalog(&mut self) {
        if let Some(w) = crate::models_cache::context_window_for(&self.config.model) {
            // Only fill if agent has not reported a max yet
            if self.context_max.is_none() {
                self.context_max = Some(w);
            }
        }
    }

    /// Enter a local, unsaved new-chat draft. No ACP session is created until
    /// the user sends the first prompt.
    fn begin_new_chat(&mut self) {
        self.begin_new_chat_in(self.config.cwd.clone());
    }

    fn begin_new_chat_in(&mut self, cwd: String) {
        if self.prompt_active() {
            self.error_banner = Some(crate::i18n::t().err_busy.into());
            return;
        }

        self.stop_stream_pump();
        self.timeline.clear();
        self.show_all_history = false;
        self.smooth_assistant.clear();
        self.smooth_thought.clear();
        self.store.on_new_chat();
        self.input.clear();
        self.slash_selected = 0;
        self.pending_images.clear();
        self.thumb_textures.clear();
        self.message_textures.clear();
        self.input_focus_request = true;
        self.scroll_to_bottom = true;
        self.scroll_to_item_id = None;
        self.focused_user_msg_id = None;
        self.needs_history_bootstrap = false;
        self.suppress_stream_updates = false;
        self.error_banner = None;

        // Drop the previously loaded agent session, but deliberately do not
        // call session/new. AcpClient::prompt_blocks creates it lazily.
        if let Some(client) = self.client.lock().clone() {
            client.clear_session();
        }

        let cwd = cwd.trim().to_string();
        self.status = crate::i18n::new_session_status(&widgets::path_short(&cwd, 28));
        self.new_chat_draft = Some(NewChatDraft { cwd });
    }

    /// Commit the local draft's workspace immediately before the first send.
    /// Returns the cwd for the prompt; errors keep the draft and input intact.
    fn commit_new_chat_draft(&mut self) -> Option<String> {
        let Some(draft) = self.new_chat_draft.as_ref() else {
            return Some(self.config.cwd.clone());
        };
        let cwd = draft.cwd.trim().to_string();
        if cwd.is_empty() {
            self.error_banner = Some(crate::i18n::t().err_need_cwd.into());
            return None;
        }
        if !std::path::Path::new(&cwd).is_dir() {
            self.error_banner = Some(crate::i18n::err_cwd_missing(&cwd));
            return None;
        }

        self.config.cwd = cwd.clone();
        let _ = self.config.save();
        self.new_chat_draft = None;
        Some(cwd)
    }

    fn stop_stream_pump(&mut self) {
        if let Some(tx) = self.stream_pump.take() {
            let _ = tx.send(());
        }
    }

    fn start_stream_pump(&mut self, ctx: &egui::Context) {
        self.stop_stream_pump();
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        self.stream_pump = Some(cancel_tx);
        let repaint = ctx.clone();
        self.rt.spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => break,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(16)) => {
                        repaint.request_repaint();
                    }
                }
            }
        });
    }

    /// All user-message ids in timeline order (oldest → newest).
    fn user_message_ids(&self) -> Vec<String> {
        self.timeline
            .iter()
            .filter_map(|i| match i {
                TimelineItem::UserMessage { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    /// Jump to previous user message (Ctrl+↑ / toolbar). From bottom → last user msg.
    fn jump_prev_user_message(&mut self) {
        let ids = self.user_message_ids();
        if ids.is_empty() {
            self.status = crate::i18n::t().status_no_user_msg.into();
            return;
        }
        let target = match &self.focused_user_msg_id {
            Some(cur) => match ids.iter().position(|x| x == cur) {
                Some(0) => {
                    self.status = crate::i18n::t().status_first_user_msg.into();
                    return;
                }
                Some(i) => ids[i - 1].clone(),
                None => ids[ids.len() - 1].clone(),
            },
            // Not focused yet: go to the most recent user message
            None => ids[ids.len() - 1].clone(),
        };
        self.focus_user_message(target);
        self.status = crate::i18n::t().status_jumped_prev.into();
    }

    /// Jump to next user message (Ctrl+↓). Past last → stick to bottom.
    fn jump_next_user_message(&mut self) {
        let ids = self.user_message_ids();
        if ids.is_empty() {
            self.status = crate::i18n::t().status_no_user_msg.into();
            return;
        }
        let target = match &self.focused_user_msg_id {
            Some(cur) => match ids.iter().position(|x| x == cur) {
                Some(i) if i + 1 < ids.len() => ids[i + 1].clone(),
                Some(_) => {
                    // After last user message → bottom of chat
                    self.focused_user_msg_id = None;
                    self.scroll_to_item_id = None;
                    self.scroll_to_bottom = true;
                    self.chat_away_from_bottom = false;
                    self.status = crate::i18n::t().status_at_latest.into();
                    return;
                }
                None => ids[ids.len() - 1].clone(),
            },
            None => {
                self.scroll_to_bottom = true;
                self.chat_away_from_bottom = false;
                self.status = crate::i18n::t().status_at_latest.into();
                return;
            }
        };
        self.focus_user_message(target);
        self.status = crate::i18n::t().status_jumped_next.into();
    }

    fn focus_user_message(&mut self, id: String) {
        // Ensure older messages aren't collapsed away
        self.show_all_history = true;
        self.focused_user_msg_id = Some(id.clone());
        self.scroll_to_item_id = Some(id);
        self.scroll_to_bottom = false;
        self.chat_away_from_bottom = true;
    }

    fn send_prompt(&mut self, ctx: &egui::Context) {
        let text = self.input.trim().to_string();
        if text.is_empty() && self.pending_images.is_empty() {
            return;
        }
        // If UI thinks we're free but agent still has a prompt RPC open, cancel first.
        let client = self.client.lock().clone();
        let Some(client) = client else {
            self.error_banner = Some(crate::i18n::t().err_not_connected.into());
            self.connect_agent(ctx);
            return;
        };
        if self.prompt_active() {
            // Still in a turn — user should stop first; do not stack prompts.
            self.error_banner = Some(crate::i18n::t().err_busy.into());
            return;
        }
        // Capture the routing contract before committing the draft. Existing
        // chats must bind this exact id; fresh drafts must force session/new.
        let target_session_id = if self.new_chat_draft.is_some() {
            None
        } else if let Some(session_id) = self.store.session_id_owned() {
            Some(session_id)
        } else {
            self.error_banner = Some(crate::i18n::session_binding_error());
            return;
        };
        let mut images = std::mem::take(&mut self.pending_images);
        self.thumb_textures.clear();
        // Persist each attachment under app data dir; send 图N:@path to agent
        for (i, img) in images.iter_mut().enumerate() {
            let n = (i + 1) as u32;
            if let Err(e) = img.ensure_on_disk(n) {
                self.error_banner = Some(format!(
                    "{} (#{n}): {e:#}",
                    crate::i18n::t().attach_persist_failed
                ));
                // put remaining back
                self.pending_images = images;
                return;
            }
        }
        let Some(cwd) = self.commit_new_chat_draft() else {
            self.pending_images = images;
            return;
        };
        let chat_images: Vec<ChatImage> = images.iter().map(|i| i.to_chat_image()).collect();
        let blocks = attachments::build_prompt_blocks(&text, &images);
        // User-visible text includes path notes so history matches what agent saw
        let display_text = if images.is_empty() {
            text.clone()
        } else {
            let mut t = text.clone();
            if !t.is_empty() {
                t.push_str("\n\n");
            }
            for img in &images {
                let n = img.label_index.max(1);
                let path = img
                    .disk_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                t.push_str(&format!(
                    "{}\n",
                    crate::i18n::image_n_path(n as usize, &path)
                ));
            }
            t
        };

        self.timeline.push(TimelineItem::UserMessage {
            id: Uuid::new_v4().to_string(),
            text: display_text.clone(),
            attachments: chat_images,
        });
        self.input.clear();
        let turn_gen = self.store.begin_turn();
        self.smooth_assistant.clear();
        self.smooth_thought.clear();
        self.status = if images.is_empty() {
            crate::i18n::t().generating_ellipsis.into()
        } else {
            crate::i18n::generating_with_n(images.len())
        };
        self.scroll_to_bottom = true;
        self.chat_away_from_bottom = false;
        self.error_banner = None;
        self.touch_session_list(&display_text);
        self.start_stream_pump(ctx);

        let bootstrap = if self.needs_history_bootstrap {
            self.needs_history_bootstrap = false;
            let turns = self.collect_history_turns();
            // exclude the message we just pushed for bootstrap? include prior only
            let prior: Vec<_> = if turns.len() > 1 {
                turns[..turns.len() - 1].to_vec()
            } else {
                Vec::new()
            };
            if prior.is_empty() {
                None
            } else {
                Some(build_history_bootstrap(&prior, 16, 14_000))
            }
        } else {
            None
        };

        let event_tx = self.event_tx.clone();
        let repaint = ctx.clone();
        self.rt.spawn(async move {
            match client
                .prompt_blocks(blocks, &cwd, bootstrap, target_session_id)
                .await
            {
                Ok(stop) => {
                    let _ = event_tx.send(AgentEvent::PromptFinished {
                        stop_reason: stop,
                        turn_gen,
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(AgentEvent::Error {
                        message: crate::i18n::prompt_failed_e(e),
                        turn_gen: Some(turn_gen),
                    });
                }
            }
            repaint.request_repaint();
        });
    }

    fn add_pending_image(&mut self, mut img: PendingImage) {
        if self.pending_images.len() >= attachments::MAX_ATTACHMENTS {
            self.error_banner = Some(crate::i18n::max_attachments(attachments::MAX_ATTACHMENTS));
            return;
        }
        // Avoid huge duplicates by name+size
        if self
            .pending_images
            .iter()
            .any(|p| p.name == img.name && p.png_bytes.len() == img.png_bytes.len())
        {
            return;
        }
        let n = (self.pending_images.len() + 1) as u32;
        if let Err(e) = img.ensure_on_disk(n) {
            self.error_banner = Some(crate::i18n::attach_save_failed_e(e));
            return;
        }
        let path_hint = img
            .disk_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.pending_images.push(img);
        self.status = crate::i18n::attached_progress(
            self.pending_images.len(),
            attachments::MAX_ATTACHMENTS,
            n as usize,
        );
        if !path_hint.is_empty() {
            self.logs.push(format!("attachment #{n} → {path_hint}"));
        }
    }

    /// App-level paste interceptor — runs before any TextEdit.
    ///
    /// CRITICAL (Windows): egui-winit intercepts Ctrl+V, calls arboard.get_text(),
    /// and on image-only clipboard it ERROR-logs and **returns without emitting
    /// Event::Key(V)** or Event::Paste. So egui never tells us about the paste.
    /// We therefore also poll the OS keyboard with GetAsyncKeyState.
    fn poll_paste_early(&mut self, ctx: &egui::Context) {
        let mut paste_texts: Vec<String> = Vec::new();
        let mut paste_event = false;
        let mut egui_ctrl_v = false;

        ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command || i.modifiers.mac_cmd;
            if ctrl && (i.key_down(Key::V) || i.key_pressed(Key::V)) {
                egui_ctrl_v = true;
            }
            for e in &i.events {
                match e {
                    egui::Event::Paste(t) => {
                        paste_event = true;
                        paste_texts.push(t.clone());
                    }
                    egui::Event::Key {
                        key: Key::V,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.ctrl || modifiers.command || modifiers.mac_cmd => {
                        egui_ctrl_v = true;
                    }
                    _ => {}
                }
            }
        });

        // OS-level edge (works even when egui-winit swallows Key::V)
        let os_ctrl_v = raw_os_ctrl_v_down();
        let ctrl_v_edge = (os_ctrl_v || egui_ctrl_v) && !self.prev_ctrl_v_down;
        self.prev_ctrl_v_down = os_ctrl_v || egui_ctrl_v;

        if ctrl_v_edge || paste_event {
            if !paste_texts.is_empty() {
                self.paste_probe_texts = paste_texts;
            }
            // Skip a couple frames so egui-winit/arboard can release the clipboard,
            // then probe for ~300ms.
            self.paste_probe_frames = 20;
            self.status = crate::i18n::t().status_reading_clipboard.into();
            self.error_banner = None;
            self.consume_paste_events(ctx);
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
            return;
        }

        if self.paste_probe_frames == 0 {
            return;
        }

        self.consume_paste_events(ctx);

        // Frames 20..18: wait only (arboard often still holds the lock)
        if self.paste_probe_frames > 17 {
            self.paste_probe_frames -= 1;
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }

        let texts = self.paste_probe_texts.clone();
        match attachments::from_clipboard_ex() {
            Ok(Some(img)) => {
                self.on_image_pasted(ctx, img);
                return;
            }
            Ok(None) => {
                // no image formats right now — keep probing (delayed render)
            }
            Err(e) => {
                // OpenClipboard busy — keep probing
                self.status = crate::i18n::clipboard_busy_e(e);
            }
        }
        for t in &texts {
            if let Some(img) = attachments::from_paste_payload(t) {
                self.on_image_pasted(ctx, img);
                return;
            }
        }

        self.paste_probe_frames = self.paste_probe_frames.saturating_sub(1);
        if self.paste_probe_frames > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }

        // Probe exhausted — fall back to text (our path, not arboard)
        if !self.paste_probe_texts.is_empty() {
            for t in self.paste_probe_texts.drain(..) {
                if !t.is_empty() {
                    self.input.push_str(&t);
                }
            }
            self.status = crate::i18n::t().status_pasted_text.into();
            self.input_focus_request = true;
            return;
        }
        if let Some(t) = attachments::clipboard_text() {
            if !t.is_empty() {
                self.input.push_str(&t);
                self.status = crate::i18n::t().status_pasted_text.into();
                self.input_focus_request = true;
                return;
            }
        }

        let probe = attachments::probe_clipboard();
        if probe.has_image {
            self.status = crate::i18n::image_read_failed_e(&probe.formats);
            self.error_banner = Some(crate::i18n::paste_image_failed(&probe.formats));
        } else {
            // Quiet: most often user hit Ctrl+V with empty / non-image clipboard
            self.status = crate::i18n::t().status_ready.into();
        }
    }

    fn on_image_pasted(&mut self, ctx: &egui::Context, img: PendingImage) {
        let label = img.summary_label();
        let n_before = self.pending_images.len();
        self.add_pending_image(img);
        self.error_banner = None;
        if self.pending_images.len() > n_before {
            self.status = crate::i18n::pasted_image_s(&label);
        }
        self.consume_paste_events(ctx);
        self.scrub_input_after_image_paste();
        self.input_focus_request = true;
        self.paste_probe_frames = 0;
        self.paste_probe_texts.clear();
        ctx.request_repaint();
    }

    fn scrub_input_after_image_paste(&mut self) {
        // Drop control / replacement chars that image-as-text paste may inject
        self.input = self
            .input
            .chars()
            .filter(|c| !c.is_control() && *c != '\u{FFFD}')
            .collect();
    }

    fn consume_paste_events(&self, ctx: &egui::Context) {
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::Paste(_)
                        | egui::Event::Text(_)
                        | egui::Event::Key {
                            key: Key::V,
                            pressed: true,
                            ..
                        }
                )
            });
        });
    }

    fn pick_image_files(&mut self) {
        let files = rfd::FileDialog::new()
            .add_filter(
                crate::i18n::t().images_filter,
                &["png", "jpg", "jpeg", "gif", "webp", "bmp"],
            )
            .pick_files();
        if let Some(paths) = files {
            for p in paths {
                match attachments::from_path(&p) {
                    Ok(img) => self.add_pending_image(img),
                    Err(e) => {
                        self.error_banner = Some(format!("{}: {e}", p.display()));
                    }
                }
            }
        }
    }

    fn handle_file_drops(&mut self, ctx: &egui::Context) {
        let drops: Vec<(
            Option<std::path::PathBuf>,
            Option<std::sync::Arc<[u8]>>,
            String,
        )> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| (f.path.clone(), f.bytes.clone(), f.name.clone()))
                .collect()
        });
        for (path, bytes, name) in drops {
            let result = if let Some(p) = path {
                attachments::from_path(&p)
            } else if let Some(b) = bytes {
                attachments::from_bytes(&b, if name.is_empty() { "drop.png" } else { &name })
            } else {
                continue;
            };
            match result {
                Ok(img) => self.add_pending_image(img),
                Err(e) => self.error_banner = Some(crate::i18n::drop_image_failed_e(e)),
            }
        }
    }

    fn ensure_thumb(
        &mut self,
        ctx: &egui::Context,
        img: &PendingImage,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.thumb_textures.get(&img.id) {
            return Some(t.clone());
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [img.width as usize, img.height as usize],
            &img.rgba,
        );
        let tex = ctx.load_texture(
            format!("att-{}", img.id),
            color,
            egui::TextureOptions::LINEAR,
        );
        self.thumb_textures.insert(img.id.clone(), tex.clone());
        Some(tex)
    }

    fn ensure_chat_image_tex(
        &mut self,
        ctx: &egui::Context,
        img: &ChatImage,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.message_textures.get(&img.id) {
            return Some(t.clone());
        }
        if img.rgba.len() != (img.width as usize) * (img.height as usize) * 4 {
            return None;
        }
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [img.width as usize, img.height as usize],
            &img.rgba,
        );
        let tex = ctx.load_texture(
            format!("msg-img-{}", img.id),
            color,
            egui::TextureOptions::LINEAR,
        );
        self.message_textures.insert(img.id.clone(), tex.clone());
        Some(tex)
    }

    fn cancel_prompt(&mut self, ctx: &egui::Context) {
        // ACP cancellation: permission cancel + session/cancel + UI unlock.
        // MUST cancel the agent turn — unlocking UI alone leaves session/prompt
        // running, so the next send races and only a short reply comes back.
        if let Some(pending) = self.store.take_pending_permission() {
            if let Some(client) = self.client.lock().clone() {
                let rid = pending.request_id;
                self.rt.spawn(async move {
                    let _ = client.cancel_permission(rid).await;
                });
            }
        }
        self.mark_running_tools_terminal("cancelled");
        self.store.clear_live_tool();
        let _ = ctx;
        self.force_unlock_ui(crate::i18n::t().status_stopped);
    }

    /// Free the UI. Always cancels the in-flight ACP prompt when a client exists —
    /// otherwise the next message double-books session/prompt and dies mid-reply.
    fn force_unlock_ui(&mut self, status: &str) {
        self.stop_stream_pump();
        // Cancel agent first if still attached
        if let Some(client) = self.client.lock().clone() {
            self.rt.spawn(async move {
                let _ = client.cancel().await;
            });
        }
        self.store.force_unlock();
        self.smooth_assistant.finish();
        self.smooth_thought.finish();
        self.finalize_open_streams();
        self.mark_running_tools_terminal("cancelled");
        self.status = status.into();
    }

    fn touch_activity(&mut self) {
        self.store.touch_activity();
    }

    fn respond_permission(&mut self, option_id: String, ctx: &egui::Context) {
        let Some(pending) = self.store.take_pending_permission() else {
            return;
        };
        if let Some(client) = self.client.lock().clone() {
            let rid = pending.request_id;
            let opt = option_id.clone();
            let repaint = ctx.clone();
            self.rt.spawn(async move {
                let _ = client.respond_permission(rid, &opt).await;
                repaint.request_repaint();
            });
        }
        self.touch_activity();
        self.status = crate::i18n::approved_tool_s(&option_id);
    }

    /// Prefer allow-* options for auto-approve.
    fn pick_allow_option(options: &[PermissionOption]) -> Option<String> {
        const PREFER: &[&str] = &[
            "allow-always",
            "allow_always",
            "allow-once",
            "allow_once",
            "allow",
            "approve",
            "yes",
        ];
        for key in PREFER {
            if let Some(o) = options
                .iter()
                .find(|o| o.option_id.eq_ignore_ascii_case(key))
            {
                return Some(o.option_id.clone());
            }
        }
        // Fall back: any option whose id/name/kind mentions allow/approve
        options
            .iter()
            .find(|o| {
                let blob = format!("{} {} {}", o.option_id, o.name, o.kind).to_ascii_lowercase();
                blob.contains("allow") || blob.contains("approve") || blob.contains("接受")
            })
            .map(|o| o.option_id.clone())
            .or_else(|| options.first().map(|o| o.option_id.clone()))
    }

    fn run_grok_login(&mut self) {
        if self.login_started {
            return;
        }
        let bin = match resolve_grok_binary(&self.config.grok_path) {
            Ok(b) => b,
            Err(e) => {
                self.error_banner = Some(e.to_string());
                return;
            }
        };
        self.onboarding_open = true;
        self.login_started = true;
        self.login_auth_stamp = auth_file_stamp();
        self.last_readiness_probe = None;
        self.logs.push(format!("启动登录: {} login", bin.display()));
        // Interactive login needs a real console window (not hidden).
        // CREATE_NEW_CONSOLE so it doesn't steal/flash the GUI's console.
        let spawn_result = {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
                std::process::Command::new(&bin)
                    .arg("login")
                    .creation_flags(CREATE_NEW_CONSOLE)
                    .spawn()
            }
            #[cfg(not(windows))]
            {
                std::process::Command::new(&bin).arg("login").spawn()
            }
        };
        match spawn_result {
            Ok(mut child) => {
                let (tx, rx) = std_mpsc::channel();
                self.login_rx = Some(rx);
                std::thread::spawn(move || {
                    let result = child
                        .wait()
                        .map_err(|e| format!("wait for grok login: {e}"))
                        .and_then(|status| {
                            if status.success() {
                                Ok(())
                            } else {
                                Err(format!("grok login exited with {status}"))
                            }
                        });
                    let _ = tx.send(result);
                });
            }
            Err(e) => {
                self.login_started = false;
                self.login_auth_stamp = None;
                self.error_banner = Some(format!("启动 grok login 失败: {e}"));
                return;
            }
        }
        self.status = crate::i18n::t().status_login_opened.into();
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        // Collect auto-permission replies to dispatch after match
        let mut auto_perms: Vec<(serde_json::Value, String)> = Vec::new();
        let mut processed = 0usize;
        // Drain more aggressively so tool-heavy turns don't lag message text
        const MAX_PER_FRAME: usize = 200;

        while processed < MAX_PER_FRAME {
            let Ok(ev) = self.event_rx.try_recv() else {
                break;
            };
            processed += 1;
            // Any agent traffic counts as progress while a turn is open
            if self.store.busy() {
                self.touch_activity();
            }
            match ev {
                AgentEvent::Connected {
                    agent_name,
                    agent_version,
                } => {
                    self.store.handshake_ok();
                    self.agent_label = if agent_version.is_empty() {
                        agent_name
                    } else {
                        format!("{agent_name} {agent_version}")
                    };
                    // Refresh PID after handshake (client already stored)
                    self.agent_pid = self.client.lock().as_ref().and_then(|c| c.child_pid());
                    self.status = crate::i18n::t().status_connected.into();
                    self.logs.push(format!("已连接: {}", self.agent_label));
                }
                AgentEvent::Usage { used, max, note } => {
                    if used.is_some() {
                        self.context_used = used;
                    }
                    if max.is_some() {
                        self.context_max = max;
                    }
                    if note.is_some() {
                        self.context_note = note;
                    }
                }
                AgentEvent::SessionCreated { session_id } => {
                    self.store.set_session_id(Some(session_id.clone()));
                    self.status = crate::i18n::session_label(&short_id(&session_id));
                    self.logs.push(format!("session: {session_id}"));
                    // App-owned index entry (not automatic CLI dump)
                    self.register_app_session(&session_id);
                }
                AgentEvent::SessionLoaded { session_id } => {
                    // The sidebar selection is authoritative. A stale async
                    // load must never replace it.
                    if self.store.session_id() == Some(session_id.as_str()) {
                        self.status = crate::i18n::session_label(&short_id(&session_id));
                    }
                    self.logs.push(format!("session loaded: {session_id}"));
                }
                AgentEvent::ModeChanged {
                    session_id,
                    mode_id,
                } => {
                    let mode = AgentMode::from_id(&mode_id);
                    // Ignore delayed updates from a session that is no longer
                    // selected; the user's current composer choice wins.
                    if session_id.as_deref().is_none()
                        || session_id.as_deref() == self.store.session_id()
                    {
                        self.config.set_agent_mode(mode);
                        let _ = self.config.save();
                        self.status = crate::i18n::mode_switched(mode);
                    }
                    self.logs
                        .push(format!("session mode: {} ({session_id:?})", mode.id()));
                }
                AgentEvent::MessageChunk { text } => {
                    if text.is_empty() {
                        continue;
                    }
                    // Skip history replay while session/load is active
                    if self.suppress_stream_updates {
                        continue;
                    }
                    // Agent is writing prose → any open tools are done (don't leave 进行中)
                    self.mark_running_tools_terminal("completed");
                    // New segment after tools/thought → reset smooth buffer (was causing
                    // half-shown / stuck drip from the previous bubble's target).
                    let new_segment = self.store.open_assistant_id().is_none();
                    self.append_assistant(&text);
                    if new_segment {
                        self.smooth_assistant.clear();
                    }
                    self.smooth_assistant.push_chunk(&text);
                    self.store.note_message_chunk();
                    self.status = crate::i18n::t().generating_ellipsis.into();
                    if !self.chat_away_from_bottom {
                        self.scroll_to_bottom = true;
                    }
                }
                AgentEvent::ThoughtChunk { text } => {
                    if text.is_empty() {
                        continue;
                    }
                    if self.suppress_stream_updates {
                        continue;
                    }
                    let new_segment = self.store.open_thought_id().is_none();
                    self.append_thought(&text);
                    if new_segment {
                        self.smooth_thought.clear();
                    }
                    self.smooth_thought.push_chunk(&text);
                    self.store.note_thought_chunk();
                    self.status = crate::i18n::t().thinking_ellipsis.into();
                    if !self.chat_away_from_bottom {
                        self.scroll_to_bottom = true;
                    }
                }
                AgentEvent::ToolCall {
                    id,
                    title,
                    kind,
                    status,
                    raw_input,
                } => {
                    if self.suppress_stream_updates {
                        continue;
                    }
                    // Close open assistant/thought so tools appear *between* message
                    // segments; clear smooth so the next chunk starts clean.
                    self.finalize_open_streams();
                    self.smooth_assistant.clear();
                    self.smooth_thought.clear();
                    let detail = raw_input
                        .map(|v| compact_json_preview(&v, 160))
                        .unwrap_or_default();
                    self.status = crate::i18n::tool_status(&title);
                    let st = crate::acp::parse::normalize_tool_status(&status);
                    self.store.note_tool_call(&title, &st);

                    // CLI often reuses toolCallId for sequential tools. If the last
                    // row for this id is already terminal (or title changed), push a
                    // new row so we don't clobber history / leave wrong state.
                    let existing = self.timeline.iter().rposition(
                        |i| matches!(i, TimelineItem::Tool { id: tid, .. } if *tid == id),
                    );
                    let mut push_new = true;
                    if let Some(idx) = existing {
                        if let TimelineItem::Tool {
                            title: t,
                            kind: k,
                            status: s,
                            detail: d,
                            ..
                        } = &mut self.timeline[idx]
                        {
                            let was_terminal = crate::acp::parse::tool_status_is_terminal(s);
                            let title_changed = !title.is_empty()
                                && !t.is_empty()
                                && t != &title
                                && !title.starts_with(t.as_str())
                                && !t.starts_with(title.as_str());
                            if was_terminal || title_changed {
                                // Leave old row as completed; open a fresh visual row.
                                if crate::acp::parse::tool_status_is_running(s) {
                                    *s = "completed".into();
                                }
                                push_new = true;
                            } else {
                                *t = title.clone();
                                *k = kind.clone();
                                *s = st.clone();
                                if !detail.is_empty() && d.is_empty() {
                                    *d = detail.clone();
                                }
                                push_new = false;
                            }
                        }
                    }
                    if push_new {
                        // Unique row key; ToolCallUpdate matches `id` or `id#…`
                        let row_id = if existing.is_some() {
                            format!("{id}#{}", Uuid::new_v4().as_simple())
                        } else {
                            id
                        };
                        self.timeline.push(TimelineItem::Tool {
                            id: row_id,
                            title,
                            kind,
                            status: st,
                            detail,
                        });
                    }
                    if !self.chat_away_from_bottom {
                        self.scroll_to_bottom = true;
                    }
                }
                AgentEvent::ToolCallUpdate {
                    id,
                    status,
                    title,
                    content_text,
                } => {
                    if self.suppress_stream_updates {
                        continue;
                    }
                    // Grok CLI reuses one toolCallId for a batch of sequential tools.
                    // Terminal status must apply to *all* rows for that call id, not only
                    // the latest — otherwise earlier rows stay forever "进行中".
                    let id_prefix = format!("{id}#");
                    let match_tid = |tid: &str| tid == id || tid.starts_with(&id_prefix);

                    if let Some(ref st_raw) = status {
                        let st = crate::acp::parse::normalize_tool_status(st_raw);
                        let terminal = crate::acp::parse::tool_status_is_terminal(&st);
                        let mut last_title = String::new();
                        for item in &mut self.timeline {
                            if let TimelineItem::Tool {
                                id: tid,
                                title: t,
                                status: s,
                                ..
                            } = item
                            {
                                if !match_tid(tid) {
                                    continue;
                                }
                                last_title = t.clone();
                                if terminal {
                                    // Close every open row in this call batch
                                    if crate::acp::parse::tool_status_is_running(s) {
                                        *s = st.clone();
                                    }
                                } else {
                                    // Non-terminal update: only the latest matching row
                                    // (handled below) — skip bulk here
                                }
                            }
                        }
                        // Non-terminal / title / content: update latest row only
                        if let Some(idx) = self.timeline.iter().rposition(|i| match i {
                            TimelineItem::Tool { id: tid, .. } => match_tid(tid),
                            _ => false,
                        }) {
                            if let TimelineItem::Tool {
                                title: t,
                                status: s,
                                detail: d,
                                ..
                            } = &mut self.timeline[idx]
                            {
                                if !terminal {
                                    if !(crate::acp::parse::tool_status_is_terminal(s)
                                        && crate::acp::parse::tool_status_is_running(&st))
                                    {
                                        *s = st.clone();
                                    }
                                }
                                if let Some(tt) = title.clone() {
                                    if !tt.is_empty() {
                                        *t = tt;
                                        last_title = t.clone();
                                    }
                                }
                                if let Some(ct) = content_text.clone() {
                                    if !ct.is_empty() {
                                        let one_line = ct.replace('\n', " ");
                                        if d.len() < 200 {
                                            if !d.is_empty() {
                                                d.push(' ');
                                            }
                                            d.push_str(&truncate_str(&one_line, 120));
                                        }
                                    }
                                }
                                if terminal {
                                    self.status = crate::i18n::tool_done_status(&last_title);
                                    self.store.clear_live_tool();
                                } else if crate::acp::parse::tool_status_is_running(s) {
                                    self.status = crate::i18n::tool_running_status(&t);
                                    self.store.note_tool_status(t, s);
                                }
                            }
                        }
                    } else {
                        // Title/content-only update → latest row
                        if let Some(idx) = self.timeline.iter().rposition(|i| match i {
                            TimelineItem::Tool { id: tid, .. } => match_tid(tid),
                            _ => false,
                        }) {
                            if let TimelineItem::Tool {
                                title: t,
                                detail: d,
                                ..
                            } = &mut self.timeline[idx]
                            {
                                if let Some(tt) = title {
                                    if !tt.is_empty() {
                                        *t = tt;
                                    }
                                }
                                if let Some(ct) = content_text {
                                    if !ct.is_empty() {
                                        let one_line = ct.replace('\n', " ");
                                        if d.len() < 200 {
                                            if !d.is_empty() {
                                                d.push(' ');
                                            }
                                            d.push_str(&truncate_str(&one_line, 120));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                AgentEvent::Plan { entries } => {
                    if self.suppress_stream_updates {
                        continue;
                    }
                    self.timeline.push(TimelineItem::Plan {
                        id: Uuid::new_v4().to_string(),
                        entries,
                    });
                    self.status = crate::i18n::t().planning_ellipsis.into();
                    if !self.chat_away_from_bottom {
                        self.scroll_to_bottom = true;
                    }
                }
                AgentEvent::PermissionRequest {
                    request_id,
                    tool_call_id,
                    title,
                    options,
                } => {
                    // With always_approve: never block the agent on a modal.
                    if self.config.agent_mode().always_approves() {
                        if let Some(opt) = Self::pick_allow_option(&options) {
                            self.logs
                                .push(format!("自动批准权限: {title} ({tool_call_id}) → {opt}"));
                            self.status = crate::i18n::auto_approved(&title);
                            auto_perms.push((request_id, opt));
                            continue;
                        }
                    }
                    self.store.set_permission(PendingPermission {
                        request_id,
                        tool_call_id,
                        title: title.clone(),
                        options,
                    });
                    self.status = crate::i18n::waiting_permission(&title);
                }
                AgentEvent::PromptFinished {
                    stop_reason,
                    turn_gen,
                } => {
                    let cancelled = stop_reason == "cancelled";
                    let st = if stop_reason == "end_turn"
                        || stop_reason == "cancelled"
                        || stop_reason.is_empty()
                    {
                        if cancelled {
                            crate::i18n::t().cancelled.to_string()
                        } else {
                            crate::i18n::t().ready.to_string()
                        }
                    } else {
                        crate::i18n::finished_with(&stop_reason)
                    };
                    if self.finish_turn_if_gen(turn_gen, st) {
                        if let Some(sid) = self.store.session_id_owned() {
                            let _ = sync_record_from_disk(&sid);
                            self.local_sessions = list_active_sessions();
                        }
                        if !self.chat_away_from_bottom {
                            self.scroll_to_bottom = true;
                        }
                        self.maybe_notify_turn_complete(ctx, cancelled);
                    } else {
                        self.logs.push(format!(
                            "忽略过期 PromptFinished gen={turn_gen} (current={})",
                            self.store.turn_gen()
                        ));
                    }
                }
                AgentEvent::TurnCompleted { stop_reason } => {
                    // Informational only. session/load can replay old
                    // turn_completed records, and this event has no turn_gen.
                    // The matching session/prompt response emits PromptFinished
                    // with a generation id and is the only event allowed to
                    // release the current turn.
                    self.logs
                        .push(format!("turn_completed observed: {stop_reason}"));
                }
                AgentEvent::Error { message, turn_gen } => {
                    let authentication_required = is_authentication_required_error(&message);
                    // Stale prompt error after user already unlocked / started a new turn
                    if let Some(g) = turn_gen {
                        if !self.store.is_turn_gen(g) && self.store.busy() {
                            let cur = self.store.turn_gen();
                            self.logs
                                .push(format!("忽略过期 Error gen={g} (current={cur}): {message}"));
                            continue;
                        }
                        if !self.store.busy() && self.store.turn_gen() != g {
                            self.logs
                                .push(format!("忽略过期 Error(已空闲) gen={g}: {message}"));
                            continue;
                        }
                    }
                    self.stop_stream_pump();
                    if turn_gen.is_none() {
                        // Connection-level errors
                        self.store.abort_connecting();
                    } else if let Some(g) = turn_gen {
                        let _ = self.store.end_turn_if_gen(g);
                    } else {
                        self.store.end_turn();
                    }
                    self.smooth_assistant.finish();
                    self.smooth_thought.finish();
                    self.finalize_open_streams();
                    self.mark_running_tools_terminal("failed");
                    self.error_banner = Some(message.clone());
                    self.status = crate::i18n::t().error_status.into();
                    self.logs.push(format!("ERROR: {message}"));
                    self.timeline.push(TimelineItem::Status {
                        id: Uuid::new_v4().to_string(),
                        text: format!("⚠ {message}"),
                    });
                    if authentication_required {
                        let current_stamp = auth_file_stamp();
                        let credentials_updated = auth_credentials_changed(
                            self.agent_auth_stamp.as_ref(),
                            current_stamp.as_ref(),
                        ) && is_cli_authenticated();
                        if credentials_updated {
                            // The login happened after this agent started. Replace
                            // the stale child immediately; the next send is usable.
                            self.runtime_auth_rejected = false;
                            self.pending_connect = true;
                            self.status = crate::i18n::t().connecting_ellipsis.into();
                            self.logs
                                .push("Agent 使用旧认证，检测到新凭据后自动重新连接".into());
                        } else {
                            // auth.json may still contain expired credentials.
                            self.runtime_auth_rejected = true;
                            self.settings.cli_status.authenticated = false;
                            self.settings.tab = SettingsTab::Cli;
                            self.onboarding_open = true;
                            self.logs
                                .push("Agent 拒绝现有认证，已切换为未登录状态".into());
                        }
                    }
                }
                AgentEvent::AgentExited { code, pid } => {
                    // Ignore stale exits from a previous agent after reconnect.
                    let current_pid = self.client.lock().as_ref().and_then(|c| c.child_pid());
                    let still_alive = self
                        .client
                        .lock()
                        .as_ref()
                        .map(|c| c.is_alive())
                        .unwrap_or(false);
                    if still_alive {
                        self.logs.push(format!(
                            "忽略旧 Agent 退出 code={code:?} pid={pid:?} (当前仍存活 pid={current_pid:?})"
                        ));
                        continue;
                    }
                    if let (Some(cur), Some(exited)) = (current_pid, pid) {
                        if cur != exited {
                            self.logs.push(format!(
                                "忽略非当前 Agent 退出 pid={exited} (current={cur})"
                            ));
                            continue;
                        }
                    }
                    self.force_unlock_ui(&format!("Agent exit {code:?}"));
                    self.agent_pid = None;
                    self.store.disconnect();
                    *self.client.lock() = None;
                    self.logs.push(self.status.clone());
                }
                AgentEvent::Log { message } => {
                    if let Some(sid) = message.strip_prefix("SESSION_LOAD_DONE:") {
                        if self.store.session_id() != Some(sid) {
                            self.logs.push(format!(
                                "忽略过期 session/load 完成: {sid} (current={:?})",
                                self.store.session_id()
                            ));
                            continue;
                        }
                        // History replay finished — keep disk timeline, accept new live turns only
                        self.suppress_stream_updates = false;
                        self.store.clear_stream_cursors();
                        self.smooth_assistant.clear();
                        self.smooth_thought.clear();
                        // Re-sync from disk in case anything leaked before suppress was set
                        if let Some(dir) = crate::local::app_index::find_cli_session_dir_public(sid)
                        {
                            let tl = load_session_timeline(&dir, 200);
                            if !tl.is_empty() {
                                self.timeline = tl;
                            }
                        } else if let Some(dir) = self.current_session_dir() {
                            let tl = load_session_timeline(&dir, 200);
                            if !tl.is_empty() {
                                self.timeline = tl;
                            }
                        }
                        self.scroll_to_bottom = true;
                        self.logs
                            .push(format!("已加载会话 {sid}（历史不重复渲染）"));
                    } else if let Some(rest) = message.strip_prefix("BOOTSTRAP_NEEDED:") {
                        let (sid, _) = rest.split_once(':').unwrap_or((rest, ""));
                        if self.store.session_id() == Some(sid) {
                            self.needs_history_bootstrap = true;
                            self.suppress_stream_updates = false;
                            self.logs.push(message);
                        } else {
                            self.logs.push(format!("忽略过期 session/load 失败: {sid}"));
                        }
                    } else if let Some(pid_s) = message.strip_prefix("AGENT_PID:") {
                        if let Ok(pid) = pid_s.trim().parse::<u32>() {
                            self.agent_pid = Some(pid);
                        }
                        self.logs.push(message);
                    } else {
                        self.logs.push(message);
                    }
                    if self.logs.len() > 500 {
                        let n = self.logs.len() - 400;
                        self.logs.drain(0..n);
                    }
                }
            }
        }

        // Auto-approve permissions after the event loop so we don't re-enter while matching
        for (rid, opt) in auto_perms {
            if let Some(client) = self.client.lock().clone() {
                let repaint = ctx.clone();
                self.rt.spawn(async move {
                    let _ = client.respond_permission(rid, &opt).await;
                    repaint.request_repaint();
                });
            }
        }

        // Wake UI immediately when we processed stream events
        if processed > 0 {
            ctx.request_repaint();
            if processed >= MAX_PER_FRAME {
                // More events likely waiting
                ctx.request_repaint_after(std::time::Duration::from_millis(8));
            }
        }
    }

    /// End any open streaming assistant/thought so the next event becomes a new row
    /// (tools land between message parts, not under the finished answer).
    fn finalize_open_streams(&mut self) {
        // Snap any open smooth buffer into the last assistant text before closing.
        if let Some(id) = self.store.open_assistant_id_owned() {
            if !self.smooth_assistant.target.is_empty() {
                if let Some(TimelineItem::AssistantMessage {
                    text, streaming, ..
                }) = self.timeline.iter_mut().find(|i| match i {
                    TimelineItem::AssistantMessage { id: mid, .. } => *mid == id,
                    _ => false,
                }) {
                    // Prefer full target if we were mid-drip
                    if self.smooth_assistant.target.len() > text.len()
                        && self.smooth_assistant.target.starts_with(text.as_str())
                    {
                        *text = self.smooth_assistant.target.clone();
                    }
                    *streaming = false;
                }
            }
        }
        self.store.set_open_assistant(None);
        self.store.set_open_thought(None);
        for item in &mut self.timeline {
            if let TimelineItem::AssistantMessage { streaming, .. } = item {
                *streaming = false;
            }
        }
    }

    fn append_assistant(&mut self, text: &str) {
        // New assistant text after tools → new bubble
        if self.store.open_assistant_id().is_none() {
            self.store.set_open_thought(None);
        }
        if let Some(id) = self.store.open_assistant_id() {
            if let Some(TimelineItem::AssistantMessage {
                text: body,
                streaming,
                ..
            }) = self.timeline.iter_mut().find(|i| match i {
                TimelineItem::AssistantMessage { id: mid, .. } => mid == id,
                _ => false,
            }) {
                body.push_str(text);
                *streaming = true;
                return;
            }
        }
        let id = Uuid::new_v4().to_string();
        self.store.set_open_assistant(Some(id.clone()));
        self.timeline.push(TimelineItem::AssistantMessage {
            id,
            text: text.to_string(),
            streaming: true,
        });
    }

    fn append_thought(&mut self, text: &str) {
        // Thought after assistant text → close assistant first (correct order)
        if self.store.open_assistant_id().is_some() && self.store.open_thought_id().is_none() {
            // Only close if we're starting a brand-new thought block after assistant
            // (keep open if continuing same thought)
        }
        if self.store.open_thought_id().is_none() {
            // If assistant is streaming and thought starts, keep both order by closing asst
            // when tools already forced finalize; for pure thought mid-turn leave asst open
            // only if thought comes first historically. Close asst so thought isn't buried.
            if self.store.open_assistant_id().is_some() {
                self.finalize_open_streams();
            }
        }
        if let Some(id) = self.store.open_thought_id() {
            if let Some(TimelineItem::Thought { text: body, .. }) =
                self.timeline.iter_mut().find(|i| match i {
                    TimelineItem::Thought { id: mid, .. } => mid == id,
                    _ => false,
                })
            {
                body.push_str(text);
                return;
            }
        }
        let id = Uuid::new_v4().to_string();
        self.store.set_open_thought(Some(id.clone()));
        self.timeline.push(TimelineItem::Thought {
            id,
            text: text.to_string(),
        });
    }

    // ---------- UI ----------

    fn ui_onboarding(&mut self, ctx: &egui::Context) {
        if !self.onboarding_open || self.settings.open {
            return;
        }
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
        self.input_focus_request = false;
        let s = crate::i18n::t();
        let installed = self.settings.cli_status.installed;
        let authenticated = installed && self.settings.cli_status.authenticated;
        let screen = ctx.screen_rect();

        egui::Area::new(egui::Id::new("onboarding_scrim"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, theme::modal_scrim());
                ui.allocate_rect(screen, egui::Sense::click());
            });

        let mut install_clicked = false;
        let mut login_clicked = false;
        let mut refresh_clicked = false;
        let mut advanced_clicked = false;
        egui::Window::new(s.onboarding_title)
            .id(egui::Id::new("win_onboarding"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .fixed_size([520.0, 440.0])
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .title_bar(false)
            .frame(
                Frame::NONE
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0, theme::BORDER()))
                    .corner_radius(16)
                    .inner_margin(Margin::same(26)),
            )
            .show(ctx, |ui| {
                ui.set_width(468.0);
                ui.horizontal(|ui| {
                    icons::grok_logo(ui, 32.0);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(s.onboarding_title)
                                .size(20.0)
                                .strong()
                                .color(theme::TEXT()),
                        );
                        ui.label(
                            RichText::new(s.onboarding_subtitle)
                                .size(12.5)
                                .color(theme::TEXT_3()),
                        );
                    });
                });
                ui.add_space(22.0);

                let readiness_card =
                    |ui: &mut Ui, title: &str, ready: bool, ok: &str, missing: &str| {
                        Frame::NONE
                            .fill(theme::SURFACE_2())
                            .stroke(Stroke::new(1.0, theme::BORDER()))
                            .corner_radius(10)
                            .inner_margin(Margin::symmetric(14, 12))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if ready { "●" } else { "○" })
                                            .size(12.0)
                                            .color(if ready {
                                                theme::SUCCESS()
                                            } else {
                                                theme::WARNING()
                                            }),
                                    );
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(title)
                                                .size(12.5)
                                                .strong()
                                                .color(theme::TEXT()),
                                        );
                                        ui.label(
                                            RichText::new(if ready { ok } else { missing })
                                                .size(11.5)
                                                .color(theme::TEXT_3()),
                                        );
                                    });
                                });
                            });
                    };

                readiness_card(
                    ui,
                    s.onboarding_cli_step,
                    installed,
                    s.onboarding_cli_ready,
                    s.onboarding_cli_missing,
                );
                ui.add_space(8.0);
                readiness_card(
                    ui,
                    s.onboarding_auth_step,
                    authenticated,
                    s.onboarding_auth_ready,
                    s.onboarding_auth_missing,
                );

                if self.login_started && !authenticated {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(s.onboarding_login_opened)
                            .size(11.5)
                            .color(theme::ACCENT()),
                    );
                }
                if self.settings.installing && !self.settings.install_logs.is_empty() {
                    ui.add_space(10.0);
                    let start = self.settings.install_logs.len().saturating_sub(3);
                    for line in &self.settings.install_logs[start..] {
                        ui.add(
                            egui::Label::new(
                                RichText::new(line)
                                    .size(10.5)
                                    .monospace()
                                    .color(theme::TEXT_3()),
                            )
                            .truncate(),
                        );
                    }
                }

                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    if !installed {
                        let label = if self.settings.installing {
                            s.onboarding_installing
                        } else {
                            s.onboarding_install
                        };
                        if primary_button(ui, label, !self.settings.installing).clicked() {
                            install_clicked = true;
                        }
                    } else if !authenticated {
                        if primary_button(ui, s.onboarding_login, true).clicked() {
                            login_clicked = true;
                        }
                    }
                    if ghost_button(ui, s.onboarding_check_again).clicked() {
                        refresh_clicked = true;
                    }
                    if quiet_link(ui, s.onboarding_advanced).clicked() {
                        advanced_clicked = true;
                    }
                });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(s.onboarding_required)
                        .size(11.0)
                        .color(theme::TEXT_3()),
                );
            });

        if install_clicked {
            self.start_cli_install();
        }
        if login_clicked {
            self.run_grok_login();
        }
        if refresh_clicked {
            self.last_readiness_probe = None;
            self.settings.cli_status = probe_status_fast(&self.config.grok_path);
        }
        if advanced_clicked {
            self.onboarding_open = false;
            self.settings.open = true;
            self.settings.tab = SettingsTab::Cli;
            self.settings.refresh_cli();
        }
    }

    fn ui_image_generation(&mut self, ctx: &egui::Context) {
        if !self.image_generation_open {
            return;
        }
        let s = crate::i18n::t();
        let modal = egui::Modal::new(egui::Id::new("modal_image_generation"))
            .backdrop_color(theme::modal_scrim())
            .frame(
                Frame::NONE
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0, theme::BORDER()))
                    .shadow(theme::card_shadow())
                    .corner_radius(18)
                    .inner_margin(Margin::same(0)),
            )
            .show(ctx, |ui| {
                ui.set_width(560.0_f32.min((ctx.screen_rect().width() - 32.0).max(320.0)));

                let mut close_clicked = false;
                let mut generate_clicked = false;
                let mut create_key_clicked = false;

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 17))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            icons::grok_logo(ui, 24.0);
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(s.image_generation_title)
                                        .size(17.0)
                                        .strong()
                                        .color(theme::TEXT()),
                                );
                                ui.label(
                                    RichText::new(s.image_generation_subtitle)
                                        .size(11.5)
                                        .color(theme::TEXT_3()),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::icon_btn(ui, IconKind::Close, s.cancel).clicked() {
                                    close_clicked = true;
                                }
                            });
                        });
                    });

                hairline(ui);

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 18))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(s.image_prompt)
                                .size(12.0)
                                .strong()
                                .color(theme::TEXT_2()),
                        );
                        ui.add_space(6.0);
                        Frame::NONE
                            .fill(theme::SURFACE_2())
                            .stroke(Stroke::new(1.0, theme::BORDER()))
                            .corner_radius(12)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.add(
                                    TextEdit::multiline(&mut self.image_prompt)
                                        .hint_text(s.image_prompt_hint)
                                        .desired_rows(4)
                                        .desired_width(f32::INFINITY)
                                        .frame(false),
                                );
                            });
                        ui.add_space(12.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(s.aspect_ratio)
                                        .size(11.0)
                                        .color(theme::TEXT_3()),
                                );
                                egui::ComboBox::from_id_salt("image_aspect_ratio")
                                    .selected_text(&self.image_aspect_ratio)
                                    .width(138.0)
                                    .show_ui(ui, |ui| {
                                        for ratio in [
                                            "auto", "1:1", "16:9", "9:16", "4:3", "3:4", "3:2",
                                            "2:3",
                                        ] {
                                            ui.selectable_value(
                                                &mut self.image_aspect_ratio,
                                                ratio.to_string(),
                                                ratio,
                                            );
                                        }
                                    });
                            });
                            ui.add_space(10.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(s.resolution)
                                        .size(11.0)
                                        .color(theme::TEXT_3()),
                                );
                                egui::ComboBox::from_id_salt("image_resolution")
                                    .selected_text(self.image_resolution.to_uppercase())
                                    .width(108.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.image_resolution,
                                            "1k".into(),
                                            "1K",
                                        );
                                        ui.selectable_value(
                                            &mut self.image_resolution,
                                            "2k".into(),
                                            "2K",
                                        );
                                    });
                            });
                        });

                        ui.add_space(14.0);
                        Frame::NONE
                            .fill(theme::SURFACE_2())
                            .stroke(Stroke::new(1.0, theme::BORDER()))
                            .corner_radius(12)
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(s.image_api_key)
                                            .size(12.0)
                                            .strong()
                                            .color(theme::TEXT_2()),
                                    );
                                    if quiet_link(ui, s.create_api_key).clicked() {
                                        create_key_clicked = true;
                                    }
                                });
                                ui.add_space(5.0);
                                ui.add(
                                    TextEdit::singleline(&mut self.image_api_key)
                                        .password(true)
                                        .hint_text(s.image_api_key_hint)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(s.image_api_key_session_only)
                                        .size(10.5)
                                        .color(theme::TEXT_3()),
                                );
                            });

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(s.api_billing_separate)
                                .size(10.5)
                                .color(theme::TEXT_3()),
                        );
                    });

                hairline(ui);

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 14))
                    .show(ui, |ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let enabled = !self.image_generating
                                && !self.image_prompt.trim().is_empty()
                                && !self.image_api_key.trim().is_empty();
                            let label = if self.image_generating {
                                s.image_generating
                            } else {
                                s.image_generate
                            };
                            if primary_button(ui, label, enabled).clicked() {
                                generate_clicked = true;
                            }
                            if ghost_button(ui, s.cancel).clicked() {
                                close_clicked = true;
                            }
                        });
                    });

                (close_clicked, create_key_clicked, generate_clicked)
            });
        let (close_clicked, create_key_clicked, generate_clicked) = modal.inner;

        // Visibility is independent from the background request. This makes
        // title-bar close, Cancel, backdrop click and Esc deterministic even
        // while an image request is still running.
        if close_clicked || modal.should_close() {
            self.image_generation_open = false;
        }

        if create_key_clicked {
            crate::spawn_util::open_url(API_KEY_URL);
        }
        if generate_clicked {
            self.start_image_generation();
        }
    }

    fn ui_share(&mut self, ctx: &egui::Context) {
        if !self.share_open {
            return;
        }

        let s = crate::i18n::t();
        let modal = egui::Modal::new(egui::Id::new("modal_share_product"))
            .backdrop_color(theme::modal_scrim())
            .frame(
                Frame::NONE
                    .fill(theme::SURFACE())
                    .stroke(Stroke::new(1.0, theme::BORDER()))
                    .shadow(theme::card_shadow())
                    .corner_radius(18)
                    .inner_margin(Margin::same(0)),
            )
            .show(ctx, |ui| {
                let available = (ctx.screen_rect().width() - 32.0).max(300.0);
                ui.set_width(520.0_f32.min(available));

                let mut close_clicked = false;
                let mut copy_clicked = false;
                let mut download_clicked = false;

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 17))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            icons::grok_logo(ui, 28.0);
                            ui.add_space(5.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(s.share_modal_title)
                                        .size(17.0)
                                        .strong()
                                        .color(theme::TEXT()),
                                );
                                ui.label(
                                    RichText::new(s.share_modal_subtitle)
                                        .size(11.5)
                                        .color(theme::TEXT_3()),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::icon_btn(ui, IconKind::Close, s.cancel).clicked() {
                                    close_clicked = true;
                                }
                            });
                        });
                    });

                hairline(ui);

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 20))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(s.share_description)
                                .size(13.5)
                                .color(theme::TEXT_2()),
                        );
                        ui.add_space(16.0);

                        let link_card = |ui: &mut Ui, title: &str, url: &str| {
                            Frame::NONE
                                .fill(theme::SURFACE_2())
                                .stroke(Stroke::new(1.0, theme::BORDER()))
                                .corner_radius(12)
                                .inner_margin(Margin::symmetric(14, 11))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new(title)
                                            .size(11.0)
                                            .strong()
                                            .color(theme::TEXT_3()),
                                    );
                                    ui.add_space(3.0);
                                    ui.hyperlink_to(
                                        RichText::new(url).size(12.0).color(theme::ACCENT()),
                                        url,
                                    );
                                });
                        };

                        link_card(ui, s.share_homepage, crate::share::HOMEPAGE_URL);
                        ui.add_space(8.0);
                        link_card(ui, s.share_download_page, crate::share::DOWNLOAD_URL);
                    });

                hairline(ui);

                Frame::NONE
                    .inner_margin(Margin::symmetric(22, 14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if self.share_copied {
                                ui.label(
                                    RichText::new(s.share_copied)
                                        .size(12.0)
                                        .color(theme::SUCCESS()),
                                );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if primary_button(
                                    ui,
                                    if self.share_copied {
                                        s.share_copied
                                    } else {
                                        s.share_copy
                                    },
                                    true,
                                )
                                .clicked()
                                {
                                    copy_clicked = true;
                                }
                                if ghost_button(ui, s.share_open_download).clicked() {
                                    download_clicked = true;
                                }
                            });
                        });
                    });

                (close_clicked, copy_clicked, download_clicked)
            });

        let (close_clicked, copy_clicked, download_clicked) = modal.inner;
        if copy_clicked {
            ctx.copy_text(crate::share::share_text(crate::i18n::current_locale()));
            self.share_copied = true;
        }
        if download_clicked {
            crate::spawn_util::open_url(crate::share::DOWNLOAD_URL);
        }
        if close_clicked || modal.should_close() {
            self.share_open = false;
            self.share_copied = false;
        }
    }

    fn open_settings(&mut self) {
        self.settings.open = true;
        self.settings.sync_from(&self.config);
        self.settings.refresh_cli();
        self.refresh_sessions();
    }

    fn ui_sidebar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // Clamp content width so labels never overflow the rail
        let side_w = ui.available_width();
        ui.set_max_width(side_w);
        ui.spacing_mut().item_spacing.y = 2.0;

        // Brand row — logo + title (reference `.sidebar-brand-row`)
        ui.horizontal(|ui| {
            ui.set_min_height(32.0);
            ui.spacing_mut().item_spacing.x = 8.0;
            icons::grok_logo(ui, 20.0);
            ui.label(
                RichText::new("Grok")
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_btn(ui, IconKind::Settings, crate::i18n::t().settings).clicked() {
                    self.open_settings();
                }
                if widgets::icon_btn(ui, IconKind::Share, crate::i18n::t().share).clicked() {
                    self.share_open = true;
                    self.share_copied = false;
                }
            });
        });
        ui.add_space(theme::SPACE_XS);

        // Nav: new chat — primary
        if nav_row(ui, IconKind::Plus, crate::i18n::t().new_chat).clicked() {
            self.begin_new_chat();
        }
        ui.add_space(6.0);
        // Session search
        let _ = search_field(
            ui,
            "sess_search",
            &mut self.session_filter,
            crate::i18n::t().search_sessions,
        );
        ui.add_space(4.0);
        // Quiet utilities — centered under search (App-first, not primary CTAs)
        ui.horizontal(|ui| {
            let w = ui.available_width();
            // Approximate pair width for centering
            let pair_w = 72.0;
            ui.add_space(((w - pair_w) * 0.5).max(0.0));
            ui.spacing_mut().item_spacing.x = 16.0;
            if quiet_link(ui, crate::i18n::t().import)
                .on_hover_text(crate::i18n::t().import_tip)
                .clicked()
            {
                self.show_import_panel = true;
                self.import_filter.clear();
                self.import_candidates = list_cli_import_candidates(120);
            }
            ui.label(RichText::new("·").size(11.0).color(theme::TEXT_3()));
            if quiet_link(ui, crate::i18n::t().archive)
                .on_hover_text(crate::i18n::t().archive_tip)
                .clicked()
            {
                self.show_archive_panel = true;
                self.archived_sessions = list_archived_sessions();
            }
        });
        ui.add_space(6.0);

        // Session list fills remaining space above footer
        let mut open_sess: Option<LocalSession> = None;
        let mut toggle_project: Option<(String, bool)> = None;
        let mut start_rename: Option<LocalSession> = None;
        let mut do_archive: Option<LocalSession> = None;
        let mut do_delete: Option<LocalSession> = None;
        let mut new_in_project: Option<String> = None; // project path for new chat
        let q = self.session_filter.trim().to_ascii_lowercase();
        let sessions: Vec<LocalSession> = if q.is_empty() {
            self.local_sessions.clone()
        } else {
            self.local_sessions
                .iter()
                .filter(|s| {
                    s.title.to_ascii_lowercase().contains(&q)
                        || s.cwd.to_ascii_lowercase().contains(&q)
                        || s.id.to_ascii_lowercase().contains(&q)
                        || s.model.to_ascii_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };
        let groups = group_sessions_by_project(&sessions, Some(&self.config.cwd));
        let current_project = normalize_project_key(&self.config.cwd);
        // App isolation: sidebar activity only from our SessionStore (not CLI live file)
        let cur_sid = self.store.session_id_owned();
        let footer_h = 96.0;
        let list_h = (ui.available_height() - footer_h).max(80.0);

        // Projects section open state (tree-l1)
        let projects_open = !self.collapsed_projects.contains("__section:projects");

        ScrollArea::vertical()
            .id_salt("sessions")
            .max_height(list_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(side_w);
                ui.set_min_width(side_w.min(ui.available_width()));
                ui.spacing_mut().item_spacing.y = 0.0;

                // ── tree-l1: 项目 ──────────────────────────────────
                ui.horizontal(|ui| {
                    ui.set_max_width(side_w);
                    let head_w = (side_w - 32.0).max(80.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(head_w, theme::TREE_L1_H),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            if tree_section_head(ui, crate::i18n::t().projects, projects_open)
                                .clicked()
                            {
                                if projects_open {
                                    self.collapsed_projects.insert("__section:projects".into());
                                } else {
                                    self.collapsed_projects.remove("__section:projects");
                                }
                            }
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_btn(
                            ui,
                            IconKind::Refresh,
                            crate::i18n::t().refresh_sessions,
                        )
                        .clicked()
                        {
                            self.refresh_sessions();
                        }
                    });
                });

                if !projects_open {
                    return;
                }

                if groups.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(side_w - 8.0);
                        ui.label(
                            RichText::new(if self.session_filter.trim().is_empty() {
                                crate::i18n::t().no_sessions
                            } else {
                                crate::i18n::t().no_match_sessions
                            })
                            .size(12.5)
                            .color(theme::TEXT_2()),
                        );
                        if self.session_filter.trim().is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(crate::i18n::t().click_new_chat)
                                    .size(11.5)
                                    .color(theme::TEXT_3()),
                            );
                        }
                    });
                    return;
                }

                for (gi, g) in groups.iter().enumerate() {
                    let is_current =
                        !current_project.is_empty() && g.key.eq_ignore_ascii_case(&current_project);
                    // Expand: current project by default; user can toggle.
                    // `__exp:key` = user forced expand; `key` = user forced collapse.
                    let collapsed = if self.collapsed_projects.contains(&g.key) {
                        true
                    } else if self
                        .collapsed_projects
                        .contains(&format!("__exp:{}", g.key))
                    {
                        false
                    } else if is_current {
                        false
                    } else {
                        groups.len() > 2
                    };

                    let count = g.sessions.len();
                    let name = widgets::truncate_chars(&g.name, 20);
                    ui.push_id(("proj", gi, g.key.as_str()), |ui| {
                        if gi > 0 {
                            ui.add_space(2.0);
                        }
                        // tree-l2 project row
                        let hdr = project_row(ui, &name, count, collapsed, is_current);
                        if hdr.clicked() {
                            toggle_project = Some((g.key.clone(), !collapsed));
                        }
                        hdr.context_menu(|ui| {
                            ui.set_min_width(140.0);
                            if ui.button(crate::i18n::t().new_chat_in_project).clicked() {
                                new_in_project = Some(g.path_display.clone());
                                ui.close_menu();
                            }
                            if ui
                                .button(if collapsed {
                                    crate::i18n::t().expand
                                } else {
                                    crate::i18n::t().collapse
                                })
                                .clicked()
                            {
                                toggle_project = Some((g.key.clone(), !collapsed));
                                ui.close_menu();
                            }
                        });
                        hdr.on_hover_text(&g.path_display);

                        if !collapsed {
                            // tree-l3-list: padding-left 12px, gap 2px
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.set_width((side_w - 12.0).max(40.0));
                                    ui.spacing_mut().item_spacing.y = theme::SESSION_ROW_GAP;
                                    if g.sessions.is_empty() {
                                        ui.label(
                                            RichText::new(crate::i18n::t().no_chats)
                                                .size(12.0)
                                                .color(theme::TEXT_3()),
                                        );
                                    }
                                    for (si, s) in g.sessions.iter().enumerate() {
                                        ui.push_id(("sess", si, s.id.as_str()), |ui| {
                                            let selected =
                                                cur_sid.as_deref() == Some(s.id.as_str());
                                            // Only App store drives activity — not CLI TUI
                                            let activity = if selected && self.pending_connect {
                                                SessionActivity::Connecting
                                            } else {
                                                self.store.sidebar_activity(&s.id, selected, false)
                                            };
                                            let title = if s.title.is_empty() {
                                                crate::i18n::t().untitled
                                            } else {
                                                s.title.as_str()
                                            };
                                            let meta = if !s.model.is_empty() {
                                                Some(s.model.as_str())
                                            } else {
                                                None
                                            };
                                            let resp =
                                                session_row(ui, title, meta, selected, activity);
                                            if resp.clicked() {
                                                open_sess = Some(s.clone());
                                            }
                                            resp.context_menu(|ui| {
                                                ui.set_min_width(150.0);
                                                if ui.button(crate::i18n::t().open).clicked() {
                                                    open_sess = Some(s.clone());
                                                    ui.close_menu();
                                                }
                                                if ui.button(crate::i18n::t().rename).clicked() {
                                                    start_rename = Some(s.clone());
                                                    ui.close_menu();
                                                }
                                                if ui.button(crate::i18n::t().archive).clicked() {
                                                    do_archive = Some(s.clone());
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new(
                                                                crate::i18n::t().delete_index_disk,
                                                            )
                                                            .color(theme::DANGER()),
                                                        )
                                                        .fill(Color32::TRANSPARENT),
                                                    )
                                                    .on_hover_text(
                                                        crate::i18n::t().delete_index_disk_tip,
                                                    )
                                                    .clicked()
                                                {
                                                    do_delete = Some(s.clone());
                                                    ui.close_menu();
                                                }
                                            });
                                        });
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });
                }
            });

        if let Some((key, collapse)) = toggle_project {
            let exp_key = format!("__exp:{key}");
            if collapse {
                self.collapsed_projects.insert(key);
                self.collapsed_projects.remove(&exp_key);
            } else {
                self.collapsed_projects.remove(&key);
                self.collapsed_projects.insert(exp_key);
            }
        }

        if let Some(s) = start_rename {
            self.rename_draft = Some((s.clone(), s.title.clone()));
        }
        if let Some(s) = do_archive {
            match archive_session(&s.id) {
                Ok(()) => {
                    if self.store.session_id() == Some(s.id.as_str()) {
                        self.timeline.clear();
                        self.store.set_session_id(None);
                    }
                    self.refresh_sessions();
                    self.status = crate::i18n::archived_status(&s.title);
                }
                Err(e) => {
                    self.error_banner = Some(crate::i18n::archive_failed_e(e));
                }
            }
        }
        if let Some(s) = do_delete {
            match delete_from_app(&s.id, true) {
                Ok(()) => {
                    if self.store.session_id() == Some(s.id.as_str()) {
                        self.timeline.clear();
                        self.store.set_session_id(None);
                    }
                    self.refresh_sessions();
                    self.status = crate::i18n::t().status_deleted.into();
                }
                Err(e) => {
                    self.error_banner = Some(crate::i18n::delete_failed_e(e));
                }
            }
        }
        if let Some(s) = open_sess {
            self.open_local_session(&s, ctx);
        }
        if let Some(path) = new_in_project {
            self.begin_new_chat_in(path);
        }
        self.focus_sessions = false;

        // Footer — reserved band so theme toggle is never clipped by the OS edge
        ui.add_space(theme::SPACE_XS);
        hairline(ui);
        ui.add_space(theme::SPACE_SM);

        // Flat status row
        let color = theme::status_color(self.store.is_connected(), self.store.is_connecting());
        let label = if self.store.is_connected() {
            crate::i18n::t().agent_connected
        } else if self.store.is_connecting() {
            crate::i18n::t().connecting_ellipsis
        } else {
            crate::i18n::t().agent_disconnected
        };
        let cwd_full = self.config.cwd.clone();
        let cwd_short = std::path::Path::new(&cwd_full)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(cwd_full.as_str())
            .to_string();
        ui.horizontal(|ui| {
            ui.set_min_height(28.0);
            ui.set_max_width(side_w);
            ui.spacing_mut().item_spacing.x = 8.0;
            status_dot(ui, color, label);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.store.is_connected() {
                    if ghost_button(ui, crate::i18n::t().connect).clicked() {
                        self.connect_agent(ctx);
                    }
                } else {
                    ui.label(
                        RichText::new(widgets::truncate_chars(&cwd_short, 12))
                            .size(11.0)
                            .color(theme::TEXT_3()),
                    )
                    .on_hover_text(&cwd_full);
                }
            });
        });
        ui.add_space(theme::SPACE_SM);

        // Theme segmented control — last element, with bottom padding so it isn't cut off
        let is_dark = self.config.dark_mode;
        let track = if theme::is_dark() {
            Color32::from_rgba_unmultiplied(255, 255, 255, 12)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 8)
        };
        let seg_w = ((side_w - 4.0) * 0.5).max(48.0);
        Frame::NONE
            .fill(track)
            .corner_radius(theme::RADIUS_SM)
            .inner_margin(Margin::same(2))
            .show(ui, |ui| {
                ui.set_max_width(side_w);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let seg = |ui: &mut Ui, active: bool, label: &str| {
                        ui.add(
                            egui::Button::new(RichText::new(label).size(11.5).color(if active {
                                theme::TEXT()
                            } else {
                                theme::TEXT_3()
                            }))
                            .fill(if active {
                                if theme::is_dark() {
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 28)
                                } else {
                                    Color32::WHITE
                                }
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(Stroke::NONE)
                            .corner_radius(theme::RADIUS_SM)
                            .min_size(egui::vec2(seg_w - 2.0, 26.0)),
                        )
                    };
                    let day = seg(ui, !is_dark, crate::i18n::t().day)
                        .on_hover_text(crate::i18n::t().day_tip);
                    let night = seg(ui, is_dark, crate::i18n::t().night)
                        .on_hover_text(crate::i18n::t().night_tip);
                    if day.clicked() && is_dark {
                        self.config.dark_mode = false;
                        theme::apply(ctx, false);
                        let _ = self.config.save();
                    }
                    if night.clicked() && !is_dark {
                        self.config.dark_mode = true;
                        theme::apply(ctx, true);
                        let _ = self.config.save();
                    }
                });
            });
        ui.add_space(theme::SPACE_MD);
    }

    fn ui_topbar(&mut self, ui: &mut Ui) {
        let edge = theme::EDGE_PAD;
        let bar_w = ui.available_width();
        let mut need_disconnect = false;
        let mut need_connect = false;
        let current_sid = self.store.session_id_owned();
        let thread_title = current_sid
            .as_deref()
            .and_then(|sid| self.local_sessions.iter().find(|s| s.id == sid))
            .map(|s| s.title.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::i18n::t().new_chat)
            .to_string();
        let active_cwd = self
            .new_chat_draft
            .as_ref()
            .map(|draft| draft.cwd.as_str())
            .unwrap_or(self.config.cwd.as_str());
        let project_name = std::path::Path::new(active_cwd)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(active_cwd)
            .to_string();

        ui.horizontal(|ui| {
            ui.set_min_height(theme::TOPBAR_H + 4.0);
            ui.set_max_width(bar_w);
            ui.spacing_mut().item_spacing.x = theme::SPACE_SM;
            ui.add_space(edge);

            if !self.sidebar_open {
                if widgets::icon_btn(ui, IconKind::Sidebar, crate::i18n::t().show_sidebar).clicked()
                {
                    self.sidebar_open = true;
                }
            } else if widgets::icon_btn(ui, IconKind::ChevronLeft, crate::i18n::t().hide_sidebar)
                .clicked()
            {
                self.sidebar_open = false;
            }

            // Thread context replaces the old model/config toolbar. Execution
            // controls now live beside the prompt, matching the Codex desktop
            // mental model: header = where you are, composer = how it runs.
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.set_max_width(if bar_w > 980.0 { 360.0 } else { 220.0 });
                ui.add(
                    egui::Label::new(
                        RichText::new(&thread_title)
                            .size(13.5)
                            .strong()
                            .color(theme::TEXT()),
                    )
                    .truncate(),
                )
                .on_hover_text(&thread_title);
                let sub = if let Some(sid) = current_sid.as_deref() {
                    format!(
                        "{project_name}  ·  {}",
                        crate::i18n::session_label(&short_id(sid))
                    )
                } else {
                    project_name.clone()
                };
                ui.add(
                    egui::Label::new(RichText::new(sub).size(11.0).color(theme::TEXT_3()))
                        .truncate(),
                )
                .on_hover_text(&self.config.cwd);
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(edge);
                ui.spacing_mut().item_spacing.x = 8.0;

                // Process menu
                ui.menu_button(
                    RichText::new(crate::i18n::t().process)
                        .size(12.0)
                        .color(theme::TEXT_2()),
                    |ui| {
                        ui.set_min_width(180.0);
                        if let Some(pid) = self.agent_pid {
                            ui.label(
                                RichText::new(format!("PID  {pid}"))
                                    .size(12.5)
                                    .monospace()
                                    .color(theme::TEXT()),
                            );
                        } else {
                            ui.label(
                                RichText::new(crate::i18n::t().no_agent_process)
                                    .size(12.5)
                                    .color(theme::TEXT_3()),
                            );
                        }
                        if !self.agent_label.is_empty() {
                            ui.label(
                                RichText::new(&self.agent_label)
                                    .size(11.5)
                                    .color(theme::TEXT_3()),
                            );
                        }
                        ui.separator();
                        if ui
                            .add_enabled(
                                !self.store.is_connecting(),
                                egui::Button::new(crate::i18n::t().reconnect),
                            )
                            .clicked()
                        {
                            need_connect = true;
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(
                                self.store.is_connected() || self.store.is_connecting(),
                                egui::Button::new(crate::i18n::t().disconnect),
                            )
                            .clicked()
                        {
                            need_disconnect = true;
                            ui.close_menu();
                        }
                        if self.store.busy()
                            && ui
                                .button(
                                    RichText::new(crate::i18n::t().force_end_turn)
                                        .color(theme::DANGER()),
                                )
                                .clicked()
                        {
                            self.force_unlock_ui(crate::i18n::t().status_force_ended);
                            ui.close_menu();
                        }
                    },
                );

                if widgets::icon_btn(ui, IconKind::Logs, crate::i18n::t().logs).clicked() {
                    self.show_logs = !self.show_logs;
                }

                // Live turn phase → single store projection (same as message rail)
                let mut phase = self.store.turn_phase();
                // Smooth drip still counts as generating even if cursor briefly cleared
                if phase == TurnPhase::Idle && self.store.busy() && self.smooth_assistant.active {
                    phase = TurnPhase::Generating;
                }
                let (label, dot, spinning) = if self.store.is_connecting() || self.pending_connect {
                    (crate::i18n::t().connecting, theme::WARNING(), true)
                } else if phase != TurnPhase::Idle {
                    let c = match phase {
                        TurnPhase::Permission => theme::WARNING(),
                        TurnPhase::Tool => theme::ACCENT(),
                        TurnPhase::Thinking => theme::TEXT_2(),
                        TurnPhase::Generating => theme::WARNING(),
                        TurnPhase::Idle => theme::TEXT_3(),
                    };
                    (phase.label(), c, true)
                } else if self.store.is_connected() {
                    (crate::i18n::t().ready, theme::SUCCESS(), false)
                } else {
                    (crate::i18n::t().offline, theme::TEXT_3(), false)
                };
                status_pill(ui, dot, label, spinning);

                // Context meter — used/max + progress bar
                let used = self
                    .context_used
                    .unwrap_or_else(|| self.estimate_context_tokens());
                let max = self.resolved_context_max();
                let win = max
                    .map(format_tokens_one)
                    .unwrap_or_else(|| crate::i18n::t().unknown.into());
                let src = if self.context_max.is_some() && self.context_used.is_some() {
                    crate::i18n::t().session_reported
                } else if crate::models_cache::context_window_for(&self.config.model).is_some() {
                    "models_cache.json"
                } else {
                    crate::i18n::t().not_read
                };
                let extra = self
                    .context_note
                    .as_ref()
                    .map(|n| format!("\n{n}"))
                    .unwrap_or_default();
                let ctx_tip = crate::i18n::context_tooltip(
                    &format_tokens_one(used),
                    &win,
                    src,
                    &self.config.model,
                    effort_label(&self.config.effort),
                    &extra,
                );
                context_meter(ui, used, max, &ctx_tip);

                // Live tool one-liner when space
                if bar_w > 1000.0 {
                    if let Some((title, st)) = self.store.live_tool() {
                        if st == "in_progress" || st == "running" || st == "pending" {
                            ui.label(
                                RichText::new(widgets::truncate_chars(title, 18))
                                    .size(11.5)
                                    .italics()
                                    .color(theme::TEXT_3()),
                            )
                            .on_hover_text(title);
                        }
                    }
                }
            });
        });
        hairline(ui);
        ui.add_space(theme::SPACE_SM);

        if need_disconnect {
            self.disconnect_agent();
        }
        if need_connect {
            self.connect_agent(ui.ctx());
        }
    }

    fn ui_main(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, theme::BG());
        ui.add_space(4.0);
        self.ui_topbar(ui);

        // Layout metrics shared by banner + chat + composer
        let view_w = ui.clip_rect().width().max(300.0);
        let content_w = view_w.min(theme::CHAT_MAX_WIDTH).max(280.0);
        let margin = ((view_w - content_w) * 0.5).max(theme::EDGE_PAD);

        if let Some(err) = self.error_banner.clone() {
            ui.add_space(6.0);
            let mut closed = false;
            ui.horizontal(|ui| {
                ui.add_space(margin);
                Frame::NONE
                    .fill(if theme::is_dark() {
                        Color32::from_rgba_unmultiplied(180, 50, 50, 55)
                    } else {
                        Color32::from_rgb(254, 226, 226)
                    })
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(content_w);
                        widgets::banner(ui, Color32::TRANSPARENT, &format!("⚠ {err}"), &mut closed);
                    });
            });
            if closed {
                self.error_banner = None;
            }
        }

        // Display-only chip cleanup (never cancels the ACP turn)
        self.heal_tool_display_only();

        // Stuck recovery banner — user decides; we do NOT auto session/cancel
        // (official ACP: only Client cancel notification ends a live turn early).
        if self.store.busy() {
            let idle = self
                .store
                .last_activity()
                .map(|t| t.elapsed())
                .or_else(|| self.store.busy_since().map(|t| t.elapsed()));
            let total = self.store.busy_since().map(|t| t.elapsed());
            let idle_stuck = idle
                .map(|d| d > std::time::Duration::from_secs(45))
                .unwrap_or(false);
            let long_stuck = total
                .map(|d| d > std::time::Duration::from_secs(180))
                .unwrap_or(false);
            if idle_stuck || long_stuck {
                ui.add_space(8.0);
                let msg = if idle_stuck {
                    crate::i18n::stuck_no_output_secs(idle.map(|d| d.as_secs()).unwrap_or(0))
                } else {
                    crate::i18n::stuck_running_secs(total.map(|d| d.as_secs()).unwrap_or(0))
                };
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    Frame::NONE
                        .fill(if theme::is_dark() {
                            Color32::from_rgba_unmultiplied(180, 140, 40, 48)
                        } else {
                            Color32::from_rgb(255, 248, 230)
                        })
                        .stroke(Stroke::new(
                            1.0,
                            if theme::is_dark() {
                                Color32::from_rgba_unmultiplied(240, 180, 60, 80)
                            } else {
                                Color32::from_rgb(245, 210, 140)
                            },
                        ))
                        .shadow(theme::card_shadow())
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_width(content_w.min(ui.available_width()));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                ui.label(RichText::new("⚠").size(14.0).color(theme::WARNING()));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(msg).size(12.5).color(theme::TEXT()),
                                    )
                                    .wrap(),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if primary_button(ui, crate::i18n::t().force_end_stuck, true)
                                        .clicked()
                                    {
                                        self.force_unlock_ui(
                                            crate::i18n::t().status_force_ended_ok,
                                        );
                                        self.logs.push("用户强制结束卡住的一轮".into());
                                    }
                                    if ghost_button(ui, crate::i18n::t().stop).clicked() {
                                        self.cancel_prompt(ui.ctx());
                                    }
                                });
                            });
                        });
                });
                ui.add_space(4.0);
            }
        }

        // Pin composer at the bottom first so it is never clipped by the chat scroll.
        let has_thumbs = !self.pending_images.is_empty();
        let mut suggestion: Option<String> = None;

        // Size to content (no exact_height) so the hint row is never clipped.
        egui::TopBottomPanel::bottom("composer_panel")
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(theme::BG()).inner_margin(Margin {
                left: 0,
                right: 0,
                top: 8,
                bottom: 14,
            }))
            .show_inside(ui, |ui| {
                let _ = has_thumbs;
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.allocate_ui_with_layout(
                        egui::vec2(content_w, ui.available_height()),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            ui.set_max_width(content_w);
                            ui.set_min_width(content_w.min(ui.available_width()));
                            self.ui_composer(ui, ctx);
                        },
                    );
                });
            });

        // Turn navigation shortcuts (Ctrl/Cmd + ↑/↓) — work even when TextEdit focused
        let want_prev_user = ctx.input(|i| {
            (i.modifiers.ctrl || i.modifiers.command)
                && (i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowLeft))
        });
        let want_next_user = ctx.input(|i| {
            (i.modifiers.ctrl || i.modifiers.command)
                && (i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowRight))
        });
        if want_prev_user {
            self.jump_prev_user_message();
        } else if want_next_user {
            self.jump_next_user_message();
        }

        let scroll_to_bottom = self.scroll_to_bottom;
        self.scroll_to_bottom = false;
        let scroll_to_item = self.scroll_to_item_id.take();
        let empty = self.timeline.is_empty();

        // Sync smooth targets from open streams only (never from closed bubbles).
        if let Some(id) = self.store.open_assistant_id() {
            if let Some(TimelineItem::AssistantMessage {
                text, streaming, ..
            }) = self.timeline.iter().find(|i| match i {
                TimelineItem::AssistantMessage { id: mid, .. } => mid == id,
                _ => false,
            }) {
                // If target diverged (segment restart), snap displayed
                if !text.starts_with(&self.smooth_assistant.target)
                    && !self.smooth_assistant.target.starts_with(text.as_str())
                {
                    self.smooth_assistant.clear();
                }
                if self.smooth_assistant.target != *text {
                    self.smooth_assistant.target = text.clone();
                }
                self.smooth_assistant.active = *streaming && self.config.smooth_stream;
                if !*streaming || !self.config.smooth_stream {
                    self.smooth_assistant.displayed = text.clone();
                    self.smooth_assistant.active = false;
                }
            }
        } else {
            // No open stream — don't keep showing a stale drip buffer
            if !self.store.busy() {
                self.smooth_assistant.clear();
            }
        }
        if let Some(id) = self.store.open_thought_id() {
            if let Some(TimelineItem::Thought { text, .. }) = self.timeline.iter().find(|i| match i
            {
                TimelineItem::Thought { id: mid, .. } => mid == id,
                _ => false,
            }) {
                if self.smooth_thought.target != *text {
                    self.smooth_thought.target = text.clone();
                }
                self.smooth_thought.active = self.store.busy() && self.config.smooth_stream;
            }
        }
        let smooth_changed = self.smooth_assistant.tick() || self.smooth_thought.tick();
        // Avoid every-frame repaint while busy (kills text selection / scroll).
        // Only drip-schedule when smooth stream still has backlog.
        if smooth_changed {
            ctx.request_repaint();
        } else if self.config.smooth_stream
            && self.smooth_assistant.active
            && !self.smooth_assistant.is_caught_up()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else if self.store.busy() {
            // Light keepalive for stuck banner / live tool — not 60fps
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // Only drip-override the *open* assistant; closed bubbles always use full text.
        let display_assistant_owned = if self.config.smooth_stream
            && self.store.open_assistant_id().is_some()
            && self.smooth_assistant.active
            && !self.smooth_assistant.displayed.is_empty()
        {
            // Safety: never show more than real target, never show unrelated buffer
            let d = &self.smooth_assistant.displayed;
            let t = &self.smooth_assistant.target;
            if t.starts_with(d.as_str()) || d.starts_with(t.as_str()) {
                Some(d.clone())
            } else {
                Some(t.clone())
            }
        } else {
            None
        };
        let live_tool = self.store.live_tool_owned();
        let open_asst = self.store.open_assistant_id_owned();

        // Ensure textures before the scroll UI (avoids borrow clash with timeline)
        let pending_imgs: Vec<ChatImage> = self
            .timeline
            .iter()
            .filter_map(|item| match item {
                TimelineItem::UserMessage { attachments, .. } => Some(attachments.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        for img in &pending_imgs {
            let _ = self.ensure_chat_image_tex(ctx, img);
        }

        // Stick only on explicit jump / first paint at bottom — never fight user scroll.
        let stick =
            scroll_to_bottom || (!empty && !self.chat_away_from_bottom && self.store.busy());
        let scroll_out = ScrollArea::vertical()
            .id_salt("timeline")
            .stick_to_bottom(stick && !self.chat_away_from_bottom)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_max_width(view_w);
                ui.set_min_width(view_w.min(ui.available_width()));

                ui.add_space(theme::SPACE_MD);
                ui.horizontal(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        ui.set_max_width(content_w);
                        ui.set_min_width(content_w.min(ui.available_width()));

                        // Stable turn status strip — pure store projection
                        let mut phase = self.store.turn_phase();
                        if phase == TurnPhase::Idle
                            && self.store.busy()
                            && self.smooth_assistant.active
                        {
                            phase = TurnPhase::Generating;
                        }
                        let elapsed = self.store.busy_since().map(|t| t.elapsed().as_secs());
                        chat_view::render_turn_status(
                            ui,
                            content_w,
                            phase,
                            self.store.live_tool(),
                            elapsed,
                        );

                        let mut preview_click: Option<ChatImage> = None;
                        let tex = &self.message_textures;
                        let identity = chat_view::ChatIdentity::from_config(&self.config);
                        chat_view::render_timeline(
                            ui,
                            &self.timeline,
                            &mut self.md_cache,
                            empty,
                            |s| suggestion = Some(s),
                            |img| preview_click = Some(img),
                            |img| tex.get(&img.id).cloned(),
                            display_assistant_owned.as_deref(),
                            open_asst.as_deref(),
                            live_tool.as_ref().map(|(t, s)| (t.as_str(), s.as_str())),
                            self.config.show_thoughts,
                            self.config.expand_tools,
                            &mut self.show_all_history,
                            identity,
                            scroll_to_item.as_deref(),
                        );
                        if let Some(img) = preview_click {
                            self.image_preview = Some(img);
                        }
                        if scroll_to_bottom {
                            ui.scroll_to_cursor(Some(Align::BOTTOM));
                        }
                    });
                });
                ui.add_space(theme::SPACE_XL);
            });

        // Track whether user scrolled away from bottom (hysteresis: 80px)
        let max_off = (scroll_out.content_size.y - scroll_out.inner_rect.height()).max(0.0);
        let at_bottom = max_off <= 1.0 || scroll_out.state.offset.y >= max_off - 80.0;
        if !scroll_to_bottom {
            // Only update away flag from user scroll — not from stick jumps
            if !at_bottom {
                self.chat_away_from_bottom = !empty;
            } else if at_bottom {
                self.chat_away_from_bottom = false;
            }
        }

        // Floating nav: only when scrolled away from bottom.
        // At bottom hide entirely (avoids orphan separator "竖线" + clutter).
        // Keyboard Ctrl+↑/↓ still works while at bottom.
        if self.chat_away_from_bottom && !empty {
            let has_user_msgs = self
                .timeline
                .iter()
                .any(|i| matches!(i, TimelineItem::UserMessage { .. }));
            let pill_h = 28.0;
            let bar_w = if has_user_msgs { 248.0 } else { 72.0 };
            let btn_pos = egui::pos2(
                scroll_out.inner_rect.center().x - bar_w * 0.5,
                scroll_out.inner_rect.bottom() - pill_h - 10.0,
            );
            let fill = if theme::is_dark() {
                Color32::from_rgba_unmultiplied(40, 40, 48, 235)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 240)
            };
            let stroke = if theme::is_dark() {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 28))
            } else {
                Stroke::new(1.0, Color32::from_black_alpha(22))
            };
            egui::Area::new(egui::Id::new("jump_nav_bar"))
                .order(egui::Order::Foreground)
                .fixed_pos(btn_pos)
                .show(ctx, |ui| {
                    Frame::NONE
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(14)
                        .inner_margin(Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                if has_user_msgs {
                                    let prev = ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!(
                                                    "↑ {}",
                                                    crate::i18n::t().jump_prev
                                                ))
                                                .size(12.0)
                                                .color(theme::TEXT_2()),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE)
                                            .min_size(egui::vec2(68.0, pill_h - 6.0)),
                                        )
                                        .on_hover_text(crate::i18n::t().jump_prev_tip);
                                    if prev.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if prev.clicked() {
                                        self.jump_prev_user_message();
                                    }
                                    let next = ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!(
                                                    "↓ {}",
                                                    crate::i18n::t().jump_next
                                                ))
                                                .size(12.0)
                                                .color(theme::TEXT_2()),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE)
                                            .min_size(egui::vec2(68.0, pill_h - 6.0)),
                                        )
                                        .on_hover_text(crate::i18n::t().jump_next_tip);
                                    if next.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if next.clicked() {
                                        self.jump_next_user_message();
                                    }
                                    // Soft gap, not a full vertical separator (was leaving a lone line at bottom)
                                    ui.add_space(6.0);
                                    ui.label(RichText::new("·").size(12.0).color(theme::TEXT_3()));
                                    ui.add_space(4.0);
                                }
                                let bot = ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(crate::i18n::t().jump_bottom)
                                                .size(12.0)
                                                .color(theme::TEXT_2()),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                        .min_size(egui::vec2(44.0, pill_h - 6.0)),
                                    )
                                    .on_hover_text(crate::i18n::t().jump_bottom_tip);
                                if bot.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if bot.clicked() {
                                    self.scroll_to_bottom = true;
                                    self.chat_away_from_bottom = false;
                                    self.focused_user_msg_id = None;
                                }
                            });
                        });
                });
        }

        if let Some(s) = suggestion {
            self.input = s;
            self.input_focus_request = true;
        }
    }

    fn ui_rename_dialog(&mut self, ctx: &egui::Context) {
        let Some((sess, draft)) = self.rename_draft.clone() else {
            return;
        };
        let mut open = true;
        let mut draft = draft;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new(crate::i18n::t().rename_session)
            .id(egui::Id::new("win_rename_session"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.label(
                    RichText::new(crate::i18n::t().session_title)
                        .size(12.0)
                        .color(theme::TEXT_3()),
                );
                ui.add_space(4.0);
                let te = ui.add(
                    TextEdit::singleline(&mut draft)
                        .desired_width(340.0)
                        .hint_text(crate::i18n::t().title_hint),
                );
                te.request_focus();
                if te.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !draft.trim().is_empty()
                {
                    save = true;
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, crate::i18n::t().save, !draft.trim().is_empty()).clicked()
                    {
                        save = true;
                    }
                    if ghost_button(ui, crate::i18n::t().cancel).clicked() {
                        cancel = true;
                    }
                });
            });
        if !open || cancel {
            self.rename_draft = None;
            return;
        }
        self.rename_draft = Some((sess.clone(), draft.clone()));
        if save {
            // Index is source of truth; disk summary is best-effort
            let title = draft.trim();
            match rename_in_index(&sess.id, title) {
                Ok(()) => {
                    let _ = rename_session(&sess.summary_path, title);
                    self.rename_draft = None;
                    self.refresh_sessions();
                    self.status = crate::i18n::t().status_renamed.into();
                }
                Err(e) => {
                    self.error_banner = Some(crate::i18n::rename_failed_e(e));
                }
            }
        }
    }

    fn ui_archive_panel(&mut self, ctx: &egui::Context) {
        if !self.show_archive_panel {
            return;
        }
        let mut open = true;
        let mut close_btn = false;
        let mut restore_id: Option<String> = None;
        let mut delete_id: Option<String> = None;
        let rows = self.archived_sessions.clone();
        let screen = ctx.screen_rect();
        let max_w = (screen.width() * 0.90).clamp(320.0, 440.0);
        let max_h = (screen.height() * 0.85).clamp(280.0, 520.0);
        egui::Window::new(crate::i18n::t().archived_sessions)
            .id(egui::Id::new("win_archive_sessions"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([380.0, 420.0_f32.min(max_h)])
            .min_size([300.0, 240.0])
            .max_size([max_w, max_h])
            .constrain(true)
            .constrain_to(screen)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::NONE
                    .fill(theme::modal_fill())
                    .stroke(theme::modal_stroke())
                    .inner_margin(Margin::symmetric(16, 14))
                    .corner_radius(12),
            )
            .show(ctx, |ui| {
                ui.set_max_width(max_w - 8.0);
                ui.label(
                    RichText::new(crate::i18n::t().archived_hint)
                        .size(12.0)
                        .color(theme::TEXT_3()),
                );
                ui.add_space(10.0);
                if rows.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(28.0);
                        ui.label(
                            RichText::new(crate::i18n::t().no_archived)
                                .size(13.0)
                                .color(theme::TEXT_3()),
                        );
                        ui.add_space(28.0);
                    });
                } else {
                    let scroll_h = (ui.available_height() - 36.0).min(max_h - 120.0).max(120.0);
                    ScrollArea::vertical()
                        .id_salt("archive_list")
                        .max_height(scroll_h)
                        .show(ui, |ui| {
                            for s in &rows {
                                let title = if s.title.trim().is_empty() {
                                    crate::i18n::t().untitled
                                } else {
                                    s.title.as_str()
                                };
                                ui.horizontal(|ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical(|ui| {
                                        ui.set_max_width((ui.available_width() - 100.0).max(80.0));
                                        ui.label(
                                            RichText::new(title).size(13.0).color(theme::TEXT()),
                                        );
                                        ui.label(
                                            RichText::new(widgets::path_short(&s.cwd, 28))
                                                .size(11.0)
                                                .color(theme::TEXT_3()),
                                        );
                                    });
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        if quiet_link(ui, crate::i18n::t().delete)
                                            .on_hover_text(crate::i18n::t().delete_disk_tip)
                                            .clicked()
                                        {
                                            delete_id = Some(s.id.clone());
                                        }
                                        if soft_action(ui, crate::i18n::t().restore).clicked() {
                                            restore_id = Some(s.id.clone());
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                                hairline(ui);
                                ui.add_space(4.0);
                            }
                        });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if soft_action(ui, crate::i18n::t().close).clicked() {
                            close_btn = true;
                        }
                    });
                });
            });
        if let Some(id) = restore_id {
            match restore_session(&id) {
                Ok(()) => {
                    self.status = crate::i18n::t().status_restored.into();
                    self.archived_sessions = list_archived_sessions();
                    self.local_sessions = list_active_sessions();
                }
                Err(e) => self.error_banner = Some(crate::i18n::restore_failed_e(e)),
            }
        }
        if let Some(id) = delete_id {
            match delete_from_app(&id, true) {
                Ok(()) => {
                    self.status = crate::i18n::t().status_deleted_archive.into();
                    self.archived_sessions = list_archived_sessions();
                    if self.store.session_id() == Some(id.as_str()) {
                        self.timeline.clear();
                        self.store.set_session_id(None);
                    }
                }
                Err(e) => self.error_banner = Some(crate::i18n::delete_failed_e(e)),
            }
        }
        if !open || close_btn {
            self.show_archive_panel = false;
        }
    }

    fn ui_import_panel(&mut self, ctx: &egui::Context) {
        if !self.show_import_panel {
            return;
        }
        let mut open = true;
        let mut close_btn = false;
        let mut import_one: Option<LocalSession> = None;
        let mut do_refresh = false;
        let q = self.import_filter.trim().to_ascii_lowercase();
        let rows: Vec<LocalSession> = if q.is_empty() {
            self.import_candidates.clone()
        } else {
            self.import_candidates
                .iter()
                .filter(|s| {
                    s.title.to_ascii_lowercase().contains(&q)
                        || s.cwd.to_ascii_lowercase().contains(&q)
                        || s.id.to_ascii_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };
        let total = self.import_candidates.len();
        let screen = ctx.screen_rect();
        let max_w = (screen.width() * 0.90).clamp(320.0, 440.0);
        let max_h = (screen.height() * 0.85).clamp(280.0, 520.0);
        egui::Window::new(crate::i18n::t().import_sessions)
            .id(egui::Id::new("win_import_cli"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([380.0, 440.0_f32.min(max_h)])
            .min_size([300.0, 240.0])
            .max_size([max_w, max_h])
            .constrain(true)
            .constrain_to(screen)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::NONE
                    .fill(theme::modal_fill())
                    .stroke(theme::modal_stroke())
                    .inner_margin(Margin::symmetric(16, 14))
                    .corner_radius(12),
            )
            .show(ctx, |ui| {
                ui.set_max_width(max_w - 8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} · {total}", crate::i18n::t().import_sessions))
                            .size(12.0)
                            .color(theme::TEXT_3()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if quiet_link(ui, crate::i18n::t().refresh).clicked() {
                            do_refresh = true;
                        }
                    });
                });
                ui.add_space(8.0);
                let _ = search_field(
                    ui,
                    "import_search",
                    &mut self.import_filter,
                    crate::i18n::t().filter_hint,
                );
                ui.add_space(10.0);
                if rows.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(28.0);
                        ui.label(
                            RichText::new(if total == 0 {
                                crate::i18n::t().no_importable
                            } else {
                                crate::i18n::t().no_match
                            })
                            .size(13.0)
                            .color(theme::TEXT_3()),
                        );
                        ui.add_space(28.0);
                    });
                } else {
                    // Group by working directory (same as main session list)
                    let groups = group_sessions_by_project(&rows, Some(&self.config.cwd));
                    let scroll_h = (ui.available_height() - 36.0).min(max_h - 160.0).max(120.0);
                    ScrollArea::vertical()
                        .id_salt("import_list")
                        .max_height(scroll_h)
                        .show(ui, |ui| {
                            for (gi, g) in groups.iter().enumerate() {
                                if gi > 0 {
                                    ui.add_space(8.0);
                                }
                                // Project / directory header
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    ui.label(
                                        RichText::new(&g.name)
                                            .size(12.5)
                                            .strong()
                                            .color(theme::TEXT_2()),
                                    );
                                    ui.label(
                                        RichText::new(format!("· {}", g.sessions.len()))
                                            .size(11.0)
                                            .color(theme::TEXT_3()),
                                    );
                                })
                                .response
                                .on_hover_text(&g.path_display);
                                ui.add_space(2.0);
                                hairline(ui);
                                ui.add_space(4.0);

                                for s in &g.sessions {
                                    let title = if s.title.trim().is_empty()
                                        || s.title == crate::i18n::t().untitled_paren
                                    {
                                        crate::i18n::t().untitled_session
                                    } else {
                                        s.title.as_str()
                                    };
                                    ui.horizontal(|ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add_space(8.0);
                                        ui.vertical(|ui| {
                                            ui.set_max_width(
                                                (ui.available_width() - 72.0).max(80.0),
                                            );
                                            ui.label(
                                                RichText::new(widgets::truncate_chars(title, 34))
                                                    .size(13.0)
                                                    .color(theme::TEXT()),
                                            );
                                            ui.label(
                                                RichText::new(short_id(&s.id))
                                                    .size(10.5)
                                                    .color(theme::TEXT_3()),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if soft_action(ui, crate::i18n::t().add).clicked() {
                                                    import_one = Some(s.clone());
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(3.0);
                                    hairline(ui);
                                    ui.add_space(3.0);
                                }
                            }
                        });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if soft_action(ui, crate::i18n::t().close).clicked() {
                            close_btn = true;
                        }
                    });
                });
            });
        if do_refresh {
            self.import_candidates = list_cli_import_candidates(120);
        }
        if let Some(s) = import_one {
            match import_cli_session(&s) {
                Ok(()) => {
                    self.status = crate::i18n::imported_status(&s.title);
                    self.import_candidates = list_cli_import_candidates(120);
                    self.local_sessions = list_active_sessions();
                }
                Err(e) => self.error_banner = Some(crate::i18n::import_failed_e(e)),
            }
        }
        if !open || close_btn {
            self.show_import_panel = false;
            self.import_filter.clear();
        }
    }

    fn ui_image_preview(&mut self, ctx: &egui::Context) {
        let Some(img) = self.image_preview.clone() else {
            return;
        };
        let mut open = true;
        let mut close_btn = false;
        let tex = self.ensure_chat_image_tex(ctx, &img);
        egui::Window::new(format!("{} · {}", crate::i18n::t().preview, img.label))
            .id(egui::Id::new(("img_preview", img.id.as_str())))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([720.0, 540.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let avail = ui.available_size_before_wrap();
                if let Some(tex) = &tex {
                    let scale = (avail.x / img.width as f32)
                        .min(avail.y.max(120.0) / img.height as f32)
                        .min(1.0)
                        .max(0.05);
                    let size = egui::vec2(img.width as f32 * scale, img.height as f32 * scale);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new((tex.id(), size)).corner_radius(6.0));
                    });
                } else {
                    ui.label(crate::i18n::t().preview_fail);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}×{}", img.width, img.height))
                            .size(12.0)
                            .color(theme::TEXT_3()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, crate::i18n::t().close).clicked() {
                            close_btn = true;
                        }
                    });
                });
            });
        if !open || close_btn {
            self.image_preview = None;
        }
    }

    fn set_agent_mode(&mut self, mode: AgentMode, ctx: &egui::Context) {
        if self.prompt_active() {
            self.error_banner = Some(crate::i18n::t().err_busy.into());
            return;
        }
        self.config.set_agent_mode(mode);
        let _ = self.config.save();
        self.status = crate::i18n::mode_switched(mode);
        self.timeline.push(TimelineItem::Status {
            id: Uuid::new_v4().to_string(),
            text: self.status.clone(),
        });

        if let Some(client) = self.client.lock().clone() {
            client.set_preferred_mode(mode);
            // Apply immediately only when the ACP process is bound to the
            // session visible in the UI. Otherwise the next exact session
            // load/new applies the preference without touching another chat.
            let selected = self.store.session_id_owned();
            let bound = client.session_id();
            if selected == bound {
                let event_tx = self.event_tx.clone();
                let repaint = ctx.clone();
                self.rt.spawn(async move {
                    if let Err(error) = client.set_mode(mode).await {
                        let _ = event_tx.send(AgentEvent::Error {
                            message: format!("session/set_mode: {error:#}"),
                            turn_gen: None,
                        });
                    }
                    repaint.request_repaint();
                });
            } else {
                self.logs.push(format!(
                    "mode {} queued until exact session binding (ui={selected:?}, acp={bound:?})",
                    mode.id()
                ));
            }
        }
        self.scroll_to_bottom = true;
    }

    fn apply_slash(&mut self, item: &slash::SlashItem, ctx: &egui::Context) {
        match item.action {
            SlashAction::InsertPrompt => {
                self.input = format!("/{} ", item.name);
                self.input_focus_request = true;
            }
            SlashAction::NewChat => {
                self.input.clear();
                self.begin_new_chat();
            }
            SlashAction::Settings => {
                self.input.clear();
                self.open_settings();
            }
            SlashAction::Logs => {
                self.input.clear();
                self.show_logs = true;
            }
            SlashAction::Status => {
                self.input.clear();
                let ctx_l = self.context_label();
                let msg = crate::i18n::status_line(
                    self.store.host_phase().label(),
                    &self.config.model,
                    effort_label(&self.config.effort),
                    &ctx_l,
                    &format!("{:?}", self.agent_pid),
                );
                self.status = msg.clone();
                self.timeline.push(TimelineItem::Status {
                    id: Uuid::new_v4().to_string(),
                    text: msg,
                });
                self.scroll_to_bottom = true;
            }
            SlashAction::ToggleYolo => {
                self.input.clear();
                let next = if self.config.agent_mode() == AgentMode::AlwaysApprove {
                    AgentMode::Normal
                } else {
                    AgentMode::AlwaysApprove
                };
                self.set_agent_mode(next, ctx);
            }
            SlashAction::ClearChat => {
                self.input.clear();
                self.timeline.clear();
                self.show_all_history = false;
                self.store.set_open_assistant(None);
                self.store.set_open_thought(None);
                self.status = crate::i18n::t().status_cleared_view.into();
            }
            SlashAction::CompactHint => {
                // Send as agent-facing slash so CLI can handle if supported
                self.input = "/compact".into();
                self.send_prompt(ctx);
            }
        }
        self.slash_selected = 0;
    }

    /// Resolve on-disk session dir for current ACP session.
    fn current_session_dir(&self) -> Option<std::path::PathBuf> {
        let sid = self.store.session_id()?;
        if let Some(s) = self.local_sessions.iter().find(|s| s.id == sid) {
            if s.path.is_dir() {
                return Some(s.path.clone());
            }
        }
        crate::local::app_index::find_cli_session_dir_public(sid)
    }

    fn ui_composer(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        self.handle_file_drops(ctx);

        let draft_mode = self.new_chat_draft.is_some();
        let draft_cwd_valid = self
            .new_chat_draft
            .as_ref()
            .map(|draft| {
                !draft.cwd.trim().is_empty() && std::path::Path::new(draft.cwd.trim()).is_dir()
            })
            .unwrap_or(true);
        let prompt_active = self.prompt_active();
        let cycle_mode_shortcut =
            ctx.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::Tab));
        let can_send = !prompt_active
            && self.store.is_connected()
            && draft_cwd_valid
            && (!self.input.trim().is_empty() || !self.pending_images.is_empty());

        let pending: Vec<PendingImage> = self.pending_images.clone();
        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let p = theme::t();
        let mut picked_model: Option<String> = None;
        let mut picked_effort: Option<String> = None;
        let mut picked_mode: Option<AgentMode> = if cycle_mode_shortcut && !prompt_active {
            Some(self.config.agent_mode().next())
        } else {
            None
        };
        let mut attach_requested = false;
        let mut generate_requested = false;
        let mut paste_requested = false;
        let mut workspace_choice: Option<String> = None;
        let mut browse_workspace = false;
        let project_path = self
            .new_chat_draft
            .as_ref()
            .map(|draft| draft.cwd.clone())
            .unwrap_or_else(|| self.config.cwd.clone());
        let project_name = std::path::Path::new(&project_path)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::i18n::t().need_project_dir)
            .to_string();
        let mut recent_projects: Vec<(String, String)> = Vec::new();
        if draft_mode {
            let mut seen = HashSet::new();
            for path in std::iter::once(project_path.clone()).chain(
                self.local_sessions
                    .iter()
                    .map(|session| session.cwd.clone()),
            ) {
                let key = normalize_project_key(&path);
                if key.is_empty() || !std::path::Path::new(&path).is_dir() || !seen.insert(key) {
                    continue;
                }
                let name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|part| part.to_str())
                    .filter(|part| !part.is_empty())
                    .unwrap_or(path.as_str())
                    .to_string();
                recent_projects.push((name, path));
                if recent_projects.len() >= 7 {
                    break;
                }
            }
        }

        // Slash palette above composer
        let mut slash_pick: Option<&'static slash::SlashItem> = None;
        let mut dismiss_slash = false;
        if let Some(filter) = slash::slash_filter(&self.input) {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                dismiss_slash = true;
            } else {
                if let Some(item) = slash::handle_keys(ctx, &filter, &mut self.slash_selected) {
                    // Enter — but don't also send as message this frame
                    slash_pick = Some(item);
                }
                if let Some(item) = slash::draw_palette(ui, &filter, &mut self.slash_selected) {
                    slash_pick = Some(item);
                }
                ui.add_space(6.0);
            }
        }
        if dismiss_slash {
            self.input.clear();
            self.slash_selected = 0;
        }
        if let Some(item) = slash_pick {
            // Block Enter-send for InsertPrompt path this frame
            self.apply_slash(item, ctx);
            return;
        }

        Frame::NONE
            .fill(if hovering_files {
                theme::SELECTED()
            } else if theme::is_dark() {
                theme::SURFACE()
            } else {
                Color32::WHITE
            })
            .shadow(if theme::is_dark() {
                egui::Shadow {
                    offset: [0, 6],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(72),
                }
            } else {
                egui::Shadow {
                    offset: [0, 5],
                    blur: 22,
                    spread: 0,
                    color: Color32::from_black_alpha(18),
                }
            })
            // A single quiet outline keeps the floating workbench distinct
            // without the heavy double-card appearance.
            .stroke(if hovering_files {
                Stroke::new(1.0, p.accent)
            } else {
                Stroke::new(1.0, p.border)
            })
            .inner_margin(Margin::symmetric(
                theme::SPACE_LG as i8,
                theme::SPACE_MD as i8,
            ))
            .corner_radius(14)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                if hovering_files {
                    ui.label(
                        RichText::new(crate::i18n::t().drop_to_attach)
                            .size(12.5)
                            .color(theme::ACCENT()),
                    );
                    ui.add_space(theme::SPACE_XS);
                }

                // A new chat stays a local draft. Its project selector lives in
                // the composer and does not create an ACP session by itself.
                let context_response = Frame::NONE
                    .fill(if draft_mode {
                        theme::SURFACE_2()
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(if draft_mode {
                        Stroke::new(1.0, theme::BORDER())
                    } else {
                        Stroke::NONE
                    })
                    .corner_radius(theme::RADIUS_PILL)
                    .inner_margin(Margin::symmetric(if draft_mode { 10 } else { 0 }, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 7.0;
                            let (icon_rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            icons::paint_in(ui, IconKind::Folder, icon_rect, theme::TEXT_2());
                            ui.label(
                                RichText::new(widgets::truncate_chars(&project_name, 28))
                                    .size(12.0)
                                    .color(if draft_cwd_valid {
                                        theme::TEXT_2()
                                    } else {
                                        theme::DANGER()
                                    }),
                            );
                            if draft_mode {
                                let (chevron_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(13.0, 13.0),
                                    egui::Sense::hover(),
                                );
                                icons::paint_in(
                                    ui,
                                    IconKind::ChevronDown,
                                    chevron_rect,
                                    theme::TEXT_3(),
                                );
                            }
                        });
                    })
                    .response
                    .interact(if draft_mode {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    })
                    .on_hover_text(if draft_mode {
                        crate::i18n::t().pick_folder
                    } else {
                        project_path.as_str()
                    });

                if draft_mode {
                    let popup_id = ui.make_persistent_id("new_chat_workspace_picker");
                    if context_response.clicked() {
                        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
                    }
                    egui::popup::popup_above_or_below_widget(
                        ui,
                        popup_id,
                        &context_response,
                        egui::AboveOrBelow::Above,
                        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_min_width(300.0);
                            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                            ui.label(
                                RichText::new(crate::i18n::t().projects)
                                    .size(11.0)
                                    .strong()
                                    .color(theme::TEXT_3()),
                            );
                            ui.add_space(4.0);
                            for (name, path) in &recent_projects {
                                let selected = normalize_project_key(path)
                                    == normalize_project_key(&project_path);
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!("{}  {}", if selected { "✓" } else { " " }, name),
                                    )
                                    .on_hover_text(path)
                                    .clicked()
                                {
                                    workspace_choice = Some(path.clone());
                                    ui.memory_mut(|memory| memory.close_popup());
                                }
                            }
                            ui.separator();
                            if ui.button(crate::i18n::t().pick_folder).clicked() {
                                browse_workspace = true;
                                ui.memory_mut(|memory| memory.close_popup());
                            }
                        },
                    );
                }
                ui.add_space(theme::SPACE_SM);

                if !pending.is_empty() {
                    // Cap attachment strip so many thumbs don't grow the panel forever.
                    ScrollArea::vertical()
                        .id_salt("composer_thumbs")
                        .max_height(theme::COMPOSER_THUMB_MAX_H)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing =
                                    egui::vec2(theme::SPACE_SM, theme::SPACE_SM);
                                let mut remove: Option<String> = None;
                                for img in &pending {
                                    ui.push_id(&img.id, |ui| {
                                        Frame::NONE
                                            .fill(theme::SURFACE_2())
                                            .stroke(Stroke::NONE)
                                            .inner_margin(Margin::symmetric(6, 4))
                                            .corner_radius(theme::RADIUS_SM)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;
                                                    if let Some(tex) = self.ensure_thumb(ctx, img) {
                                                        let max = 36.0_f32;
                                                        let scale = (max
                                                            / img.width.max(img.height) as f32)
                                                            .min(1.0);
                                                        let size = egui::vec2(
                                                            img.width as f32 * scale,
                                                            img.height as f32 * scale,
                                                        );
                                                        let ir = ui.add(
                                                            egui::Image::new((tex.id(), size))
                                                                .corner_radius(5.0)
                                                                .sense(egui::Sense::click()),
                                                        );
                                                        if ir.clicked() {
                                                            self.image_preview =
                                                                Some(img.to_chat_image());
                                                        }
                                                    }
                                                    ui.label(
                                                        RichText::new(widgets::path_short(
                                                            &img.name, 12,
                                                        ))
                                                        .size(11.5)
                                                        .color(theme::TEXT_2()),
                                                    );
                                                    if ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new("×")
                                                                    .size(13.0)
                                                                    .color(theme::TEXT_3()),
                                                            )
                                                            .frame(false)
                                                            .min_size(egui::vec2(20.0, 20.0)),
                                                        )
                                                        .clicked()
                                                    {
                                                        remove = Some(img.id.clone());
                                                    }
                                                });
                                            });
                                    });
                                }
                                if let Some(id) = remove {
                                    self.pending_images.retain(|x| x.id != id);
                                    self.thumb_textures.remove(&id);
                                }
                            });
                        });
                    ui.add_space(theme::SPACE_SM);
                }

                let hint = if !self.store.is_connected() {
                    crate::i18n::t().input_connect_first
                } else if pending.is_empty() {
                    crate::i18n::t().input_placeholder
                } else {
                    crate::i18n::t().input_optional_note
                };

                // Grow with content up to COMPOSER_TEXT_MAX_H, then scroll inside.
                // Bare multiline TextEdit + content-sized bottom panel previously grew without limit.
                let line_count = self.input.chars().filter(|&c| c == '\n').count() + 1;
                let min_h = theme::COMPOSER_TEXT_H;
                let max_h = theme::COMPOSER_TEXT_MAX_H;
                // Layout rows: at least 2; content-driven (ScrollArea clips outer height).
                let desired_rows = line_count.clamp(2, 200);

                let te = TextEdit::multiline(&mut self.input)
                    .desired_width(f32::INFINITY)
                    .desired_rows(desired_rows)
                    .frame(false)
                    .hint_text(RichText::new(hint).size(14.0).color(theme::TEXT_3()))
                    .return_key(None);

                let resp = ScrollArea::vertical()
                    .id_salt("composer_input")
                    .max_height(max_h)
                    .min_scrolled_height(min_h)
                    .auto_shrink([false, true])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(te)
                    })
                    .inner;

                if self.input_focus_request {
                    resp.request_focus();
                    self.input_focus_request = false;
                }
                if resp.has_focus()
                    && ctx.input(|i| {
                        (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::V)
                    })
                {
                    if self.paste_probe_frames == 0 {
                        self.paste_probe_frames = 6;
                    }
                    ctx.request_repaint();
                }
                // Don't send on Enter while slash palette is open (handled above)
                let slash_open = slash::slash_filter(&self.input).is_some();
                if resp.has_focus() && !slash_open {
                    let want_send = ctx.input(|i| {
                        if !i.key_pressed(Key::Enter) {
                            return false;
                        }
                        if self.config.enter_to_send {
                            !i.modifiers.shift
                        } else {
                            i.modifiers.ctrl || i.modifiers.command
                        }
                    });
                    if want_send {
                        if self.input.ends_with('\n') {
                            self.input.pop();
                            if self.input.ends_with('\r') {
                                self.input.pop();
                            }
                        }
                        self.send_prompt(ctx);
                    }
                }

                ui.add_space(theme::SPACE_SM);
                // One low-noise toolbar: add actions on the left, runtime choices
                // and a circular send control on the right.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.set_min_height(theme::BTN_H_LG);

                    let add_response =
                        widgets::icon_btn(ui, IconKind::Plus, crate::i18n::t().attach_tip);
                    let add_popup_id = ui.make_persistent_id("composer_add_actions");
                    if add_response.clicked() {
                        ui.memory_mut(|memory| memory.toggle_popup(add_popup_id));
                    }
                    egui::popup::popup_above_or_below_widget(
                        ui,
                        add_popup_id,
                        &add_response,
                        egui::AboveOrBelow::Above,
                        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_min_width(190.0);
                            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                            if ui
                                .button(
                                    crate::i18n::t()
                                        .attach_tip
                                        .split('·')
                                        .next()
                                        .unwrap_or(crate::i18n::t().attach_tip)
                                        .trim(),
                                )
                                .clicked()
                            {
                                attach_requested = true;
                                ui.memory_mut(|memory| memory.close_popup());
                            }
                            if self.clipboard_image_ready
                                && ui.button(crate::i18n::t().paste_image).clicked()
                            {
                                paste_requested = true;
                                ui.memory_mut(|memory| memory.close_popup());
                            }
                            if ui.button(crate::i18n::t().generate_image).clicked() {
                                generate_requested = true;
                                ui.memory_mut(|memory| memory.close_popup());
                            }
                        },
                    );

                    if !pending.is_empty() {
                        ui.label(
                            RichText::new(format!("{}", pending.len()))
                                .size(12.0)
                                .color(theme::TEXT_3()),
                        );
                    }

                    let current_mode = self.config.agent_mode();
                    let permission_label = crate::i18n::agent_mode_label(current_mode);
                    let (dot_rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 20.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        dot_rect.center(),
                        3.0,
                        match current_mode {
                            AgentMode::Normal => theme::TEXT_3(),
                            AgentMode::Plan => theme::ACCENT(),
                            AgentMode::AlwaysApprove => theme::WARNING(),
                        },
                    );
                    ui.menu_button(
                        RichText::new(permission_label)
                            .size(11.5)
                            .color(match current_mode {
                                AgentMode::Normal => theme::TEXT_3(),
                                AgentMode::Plan => theme::ACCENT(),
                                AgentMode::AlwaysApprove => theme::WARNING(),
                            }),
                        |ui| {
                            ui.set_min_width(236.0);
                            for mode in AgentMode::ALL {
                                let selected = current_mode == mode;
                                if ui
                                    .selectable_label(
                                        selected,
                                        RichText::new(crate::i18n::agent_mode_label(mode))
                                            .size(12.5)
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    picked_mode = Some(mode);
                                    ui.close_menu();
                                }
                                ui.label(
                                    RichText::new(crate::i18n::agent_mode_description(mode))
                                        .size(11.0)
                                        .color(theme::TEXT_3()),
                                );
                                if mode != AgentMode::AlwaysApprove {
                                    ui.add_space(5.0);
                                }
                            }
                        },
                    )
                    .response
                    .on_hover_text(crate::i18n::mode_switch_hint());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let send_size = 38.0;
                        if prompt_active {
                            let (stop_rect, stop_response) = ui.allocate_exact_size(
                                egui::vec2(send_size, send_size),
                                egui::Sense::click(),
                            );
                            if ui.is_rect_visible(stop_rect) {
                                ui.painter().circle_filled(
                                    stop_rect.center(),
                                    19.0,
                                    theme::SEND_BTN(),
                                );
                                ui.painter().rect_filled(
                                    egui::Rect::from_center_size(
                                        stop_rect.center(),
                                        egui::vec2(10.0, 10.0),
                                    ),
                                    2.0,
                                    theme::ON_ACCENT(),
                                );
                            }
                            if stop_response.clicked() {
                                self.cancel_prompt(ctx);
                            }
                            stop_response.on_hover_text(crate::i18n::t().stop);
                        } else {
                            let fill = if can_send {
                                theme::SEND_BTN()
                            } else {
                                theme::SURFACE_2()
                            };
                            let icon_c = if can_send {
                                theme::ON_ACCENT()
                            } else {
                                theme::TEXT_3()
                            };
                            let (srect, r) = ui.allocate_exact_size(
                                egui::vec2(send_size, send_size),
                                egui::Sense::click(),
                            );
                            if ui.is_rect_visible(srect) {
                                ui.painter()
                                    .circle_filled(srect.center(), send_size * 0.5, fill);
                                icons::paint_in(ui, IconKind::Send, srect.shrink(10.0), icon_c);
                            }
                            if r.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                ui.painter().circle_stroke(
                                    srect.center(),
                                    send_size * 0.5,
                                    Stroke::new(
                                        1.0,
                                        if can_send {
                                            theme::ACCENT()
                                        } else {
                                            theme::BORDER()
                                        },
                                    ),
                                );
                            }
                            if r.clicked() && can_send {
                                self.send_prompt(ctx);
                            }
                            r.on_hover_text(crate::i18n::t().send_tip);
                        }

                        ui.menu_button(
                            RichText::new(crate::i18n::effort_chip(&effort_label(
                                &self.config.effort,
                            )))
                            .size(11.5)
                            .color(theme::TEXT_2()),
                            |ui| {
                                ui.set_min_width(170.0);
                                ui.label(
                                    RichText::new(crate::i18n::t().effort_heading)
                                        .size(11.0)
                                        .color(theme::TEXT_3()),
                                );
                                for (id, label) in crate::config::effort_choices() {
                                    if ui
                                        .selectable_label(
                                            normalize_effort(&self.config.effort) == id,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        picked_effort = Some(id.to_string());
                                        ui.close_menu();
                                    }
                                }
                            },
                        )
                        .response
                        .on_hover_text(crate::i18n::t().effort_hint);
                        ui.menu_button(
                            RichText::new(widgets::truncate_chars(&self.config.model, 14))
                                .size(11.5)
                                .color(theme::TEXT_2()),
                            |ui| {
                                ui.set_min_width(170.0);
                                ui.label(
                                    RichText::new(crate::i18n::t().model)
                                        .size(11.0)
                                        .color(theme::TEXT_3()),
                                );
                                for model in MODELS {
                                    if ui
                                        .selectable_label(self.config.model == *model, *model)
                                        .clicked()
                                    {
                                        picked_model = Some((*model).to_string());
                                        ui.close_menu();
                                    }
                                }
                            },
                        )
                        .response
                        .on_hover_text(crate::i18n::t().model);
                    });
                });
            });

        if let Some(path) = workspace_choice {
            if let Some(draft) = self.new_chat_draft.as_mut() {
                draft.cwd = path;
            }
            self.input_focus_request = true;
        }
        if browse_workspace {
            if let Some(dir) = rfd::FileDialog::new()
                .set_directory(&project_path)
                .pick_folder()
            {
                if let Some(draft) = self.new_chat_draft.as_mut() {
                    draft.cwd = dir.display().to_string();
                }
                self.input_focus_request = true;
            }
        }
        if attach_requested {
            self.pick_image_files();
        }
        if generate_requested {
            self.image_generation_open = true;
        }
        if paste_requested {
            self.paste_probe_frames = 10;
            self.status = crate::i18n::t().status_reading_clipboard.into();
            ctx.request_repaint();
        }

        if let Some(mode) = picked_mode {
            self.set_agent_mode(mode, ctx);
        }

        let reconnect = self.store.is_connected() || self.store.is_connecting();
        if let Some(model) = picked_model {
            if model != self.config.model {
                self.config.model = model;
                crate::models_cache::invalidate_models_cache();
                self.context_max = crate::models_cache::context_window_for(&self.config.model);
                self.context_used = None;
                let _ = self.config.save();
                if reconnect {
                    self.connect_agent(ctx);
                }
            }
        }
        if let Some(effort) = picked_effort {
            if effort != normalize_effort(&self.config.effort) {
                self.config.effort = effort;
                let _ = self.config.save();
                if reconnect {
                    self.connect_agent(ctx);
                }
            }
        }
    }

    fn ui_logs(&mut self, ctx: &egui::Context) {
        if !self.show_logs {
            return;
        }
        let mut open = self.show_logs;
        egui::Window::new(crate::i18n::t().runtime_logs)
            .id(egui::Id::new("win_run_logs"))
            .open(&mut open)
            .default_size([580.0, 380.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ghost_button(ui, crate::i18n::t().clear_logs).clicked() {
                        self.logs.clear();
                    }
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("run_logs_scroll")
                    .show(ui, |ui| {
                        for (i, line) in self.logs.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.label(
                                    RichText::new(line)
                                        .monospace()
                                        .size(12.0)
                                        .color(theme::TEXT_MUTED()),
                                );
                            });
                        }
                    });
            });
        self.show_logs = open;
    }

    /// Formal update modal: changelog + download / later.
    /// "Later" closes the modal but keeps the bottom-left badge.
    fn ui_update_modal(&mut self, ctx: &egui::Context) {
        if !self.update.modal_open {
            return;
        }

        let screen = ctx.screen_rect();
        // Scrim
        egui::Area::new(egui::Id::new("update_scrim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .sense(egui::Sense::click())
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, theme::modal_scrim());
                let resp = ui.allocate_rect(screen, egui::Sense::click());
                if resp.clicked() {
                    // Click outside = Later (keep badge)
                    ui.ctx().memory_mut(|m| {
                        m.data
                            .insert_temp(egui::Id::new("update_scrim_clicked"), true);
                    });
                }
            });

        if ctx.memory(|m| {
            m.data
                .get_temp::<bool>(egui::Id::new("update_scrim_clicked"))
                .unwrap_or(false)
        }) {
            ctx.memory_mut(|m| {
                m.data.remove::<bool>(egui::Id::new("update_scrim_clicked"));
            });
            self.dismiss_update_modal();
            return;
        }

        let s = crate::i18n::t();
        let latest_tag = self
            .update
            .latest
            .as_ref()
            .map(|r| r.tag.clone())
            .unwrap_or_default();
        let current = self.update.current.clone();
        let body = self
            .update
            .selected_release()
            .map(|r| {
                if r.body.trim().is_empty() {
                    r.name.clone()
                } else {
                    r.body.clone()
                }
            })
            .unwrap_or_default();
        let release_name = self
            .update
            .selected_release()
            .map(|r| {
                if r.name.trim().is_empty() {
                    r.tag.clone()
                } else {
                    r.name.clone()
                }
            })
            .unwrap_or_else(|| latest_tag.clone());
        let history: Vec<(String, String)> = self
            .update
            .history
            .iter()
            .map(|r| (r.tag.clone(), r.name.clone()))
            .collect();
        let selected = self
            .update
            .selected_tag
            .clone()
            .unwrap_or_else(|| latest_tag.clone());

        let mut open = true;
        let mut do_later = false;
        let mut do_download = false;
        let mut do_releases = false;
        let mut pick_tag: Option<String> = None;

        egui::Window::new(s.update_modal_title)
            .id(egui::Id::new("win_update"))
            .open(&mut open)
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 480.0])
            .min_size([400.0, 320.0])
            .max_size([720.0, screen.height() * 0.88])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::NONE
                    .fill(theme::modal_fill())
                    .stroke(theme::modal_stroke())
                    .inner_margin(Margin::symmetric(22, 18)),
            )
            .show(ctx, |ui| {
                // Version strip
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    Frame::NONE
                        .fill(if theme::is_dark() {
                            Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                        } else {
                            Color32::from_rgb(0xF0, 0xF0, 0xF3)
                        })
                        .corner_radius(theme::RADIUS_SM)
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{}  v{current}", s.update_current))
                                    .size(12.5)
                                    .color(theme::TEXT_2()),
                            );
                        });
                    ui.label(RichText::new("→").size(14.0).color(theme::TEXT_3()));
                    Frame::NONE
                        .fill(Color32::from_rgba_unmultiplied(
                            theme::ACCENT().r(),
                            theme::ACCENT().g(),
                            theme::ACCENT().b(),
                            if theme::is_dark() { 36 } else { 28 },
                        ))
                        .corner_radius(theme::RADIUS_SM)
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{}  {latest_tag}", s.update_latest))
                                    .size(12.5)
                                    .strong()
                                    .color(theme::ACCENT()),
                            );
                        });
                });

                ui.add_space(12.0);
                ui.label(
                    RichText::new(&release_name)
                        .size(15.0)
                        .strong()
                        .color(theme::TEXT()),
                );

                // History picker when multiple releases
                if history.len() > 1 {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (tag, _name) in &history {
                            let active = tag == &selected;
                            let resp = ui.add(
                                egui::Button::new(RichText::new(tag).size(11.5).color(if active {
                                    theme::ON_ACCENT()
                                } else {
                                    theme::TEXT_2()
                                }))
                                .fill(if active {
                                    theme::ACCENT()
                                } else if theme::is_dark() {
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                                } else {
                                    Color32::from_rgb(0xEE, 0xEE, 0xF2)
                                })
                                .stroke(Stroke::NONE)
                                .corner_radius(theme::RADIUS_PILL)
                                .min_size(egui::vec2(0.0, 24.0)),
                            );
                            if resp.clicked() {
                                pick_tag = Some(tag.clone());
                            }
                        }
                    });
                }

                ui.add_space(10.0);
                ui.label(
                    RichText::new(s.update_changelog)
                        .size(12.0)
                        .color(theme::TEXT_3()),
                );
                ui.add_space(4.0);

                let log_h = (ui.available_height() - 56.0).clamp(160.0, 360.0);
                Frame::NONE
                    .fill(if theme::is_dark() {
                        theme::SURFACE_2()
                    } else {
                        Color32::from_rgb(0xF7, 0xF7, 0xF9)
                    })
                    .stroke(Stroke::new(1.0, theme::DIVIDER()))
                    .corner_radius(theme::RADIUS_SM)
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("update_changelog")
                            .max_height(log_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                // Plain text changelog (markdown-ish from GitHub)
                                if body.trim().is_empty() {
                                    ui.label(RichText::new("—").size(13.0).color(theme::TEXT_3()));
                                } else {
                                    for line in body.lines() {
                                        let t = line.trim_end();
                                        if t.starts_with("### ") {
                                            ui.add_space(6.0);
                                            ui.label(
                                                RichText::new(t.trim_start_matches("### ").trim())
                                                    .size(13.5)
                                                    .strong()
                                                    .color(theme::TEXT()),
                                            );
                                        } else if t.starts_with("## ") {
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new(t.trim_start_matches("## ").trim())
                                                    .size(14.5)
                                                    .strong()
                                                    .color(theme::TEXT()),
                                            );
                                        } else if t.starts_with("# ") {
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(t.trim_start_matches("# ").trim())
                                                    .size(15.0)
                                                    .strong()
                                                    .color(theme::TEXT()),
                                            );
                                        } else if t.starts_with("- ") || t.starts_with("* ") {
                                            ui.label(
                                                RichText::new(format!("• {}", t[2..].trim()))
                                                    .size(13.0)
                                                    .color(theme::TEXT_2()),
                                            );
                                        } else if t.is_empty() {
                                            ui.add_space(4.0);
                                        } else {
                                            ui.label(
                                                RichText::new(t).size(13.0).color(theme::TEXT_2()),
                                            );
                                        }
                                    }
                                }
                            });
                    });

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ghost_button(ui, s.update_open_releases).clicked() {
                        do_releases = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, s.update_download, true).clicked() {
                            do_download = true;
                        }
                        if ghost_button(ui, s.update_later).clicked() {
                            do_later = true;
                        }
                    });
                });
            });

        if !open || do_later {
            self.dismiss_update_modal();
        }
        if let Some(tag) = pick_tag {
            self.update.selected_tag = Some(tag);
        }
        if do_download {
            self.open_update_download();
        }
        if do_releases {
            update::open_url(update::RELEASES_URL);
        }
    }

    /// Bottom-left corner reminder while an update is available (survives "Later").
    fn ui_update_badge(&mut self, ctx: &egui::Context) {
        if !self.update.show_corner_badge() || self.update.modal_open {
            return;
        }
        let s = crate::i18n::t();
        let tag = self
            .update
            .latest
            .as_ref()
            .map(|r| r.tag.as_str())
            .unwrap_or("");
        let label = format!("{}  {tag}", s.update_badge);

        let mut open_modal = false;
        let mut download = false;

        // Offset from OS edge / taskbar
        let x_off = 12.0;
        let y_off = -14.0;

        egui::Area::new(egui::Id::new("update_corner_badge"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_BOTTOM, [x_off, y_off])
            .interactable(true)
            .show(ctx, |ui| {
                let fill = if theme::is_dark() {
                    Color32::from_rgb(0x1A, 0x24, 0x36)
                } else {
                    Color32::from_rgb(0xEE, 0xF4, 0xFF)
                };
                let stroke = Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        theme::ACCENT().r(),
                        theme::ACCENT().g(),
                        theme::ACCENT().b(),
                        if theme::is_dark() { 140 } else { 100 },
                    ),
                );
                let frame_resp = Frame::NONE
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(theme::RADIUS_PILL)
                    .inner_margin(Margin::symmetric(12, 7))
                    .shadow(if theme::is_dark() {
                        egui::Shadow {
                            offset: [0, 2],
                            blur: 12,
                            spread: 0,
                            color: Color32::from_black_alpha(90),
                        }
                    } else {
                        egui::Shadow {
                            offset: [0, 2],
                            blur: 10,
                            spread: 0,
                            color: Color32::from_black_alpha(28),
                        }
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let (dot_rect, _) =
                                ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(dot_rect.center(), 3.5, theme::ACCENT());
                            ui.label(
                                RichText::new(&label)
                                    .size(12.5)
                                    .strong()
                                    .color(theme::TEXT()),
                            );
                            let view = ui.add(
                                egui::Button::new(
                                    RichText::new(s.update_view)
                                        .size(11.5)
                                        .color(theme::ON_ACCENT()),
                                )
                                .fill(theme::ACCENT())
                                .stroke(Stroke::NONE)
                                .corner_radius(theme::RADIUS_PILL)
                                .min_size(egui::vec2(44.0, 22.0)),
                            );
                            if view.clicked() {
                                open_modal = true;
                            }
                            let dl = ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(s.update_download)
                                            .size(11.0)
                                            .color(theme::ACCENT()),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(egui::vec2(0.0, 22.0)),
                                )
                                .on_hover_text(s.update_download);
                            if dl.clicked() {
                                download = true;
                            }
                        });
                    })
                    .response
                    .on_hover_text(format!(
                        "{}\n{} → {}",
                        s.update_available,
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                        tag
                    ));

                if frame_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if frame_resp.clicked() && !open_modal && !download {
                    open_modal = true;
                }
            });

        if open_modal {
            self.update.modal_open = true;
            if let Some(t) = self.update.latest.as_ref().map(|r| r.tag.clone()) {
                self.update.selected_tag = Some(t);
            }
        }
        if download {
            self.open_update_download();
        }
    }

    fn ui_permission_modal(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.store.pending_permission() else {
            return;
        };
        let title = pending.title.clone();
        let tool_id = pending.tool_call_id.clone();
        let options = pending.options.clone();

        egui::Window::new(crate::i18n::t().tool_permission)
            .id(egui::Id::new("win_tool_permission"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.label(RichText::new(&title).strong().size(15.0));
                ui.label(
                    RichText::new(format!("id: {tool_id}"))
                        .size(11.0)
                        .color(theme::TEXT_DIM()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for opt in &options {
                        let is_allow = opt.kind.contains("allow");
                        if is_allow {
                            if primary_button(ui, &opt.name, true).clicked() {
                                self.respond_permission(opt.option_id.clone(), ctx);
                            }
                        } else if ghost_button(ui, &opt.name).clicked() {
                            self.respond_permission(opt.option_id.clone(), ctx);
                        }
                    }
                    if options.is_empty() {
                        if primary_button(ui, crate::i18n::t().allow_once, true).clicked() {
                            self.respond_permission("allow-once".into(), ctx);
                        }
                        if ghost_button(ui, crate::i18n::t().deny).clicked() {
                            self.respond_permission("reject-once".into(), ctx);
                        }
                    }
                });
            });
    }
}

fn short_id(id: &str) -> String {
    if id.len() <= 10 {
        id.to_string()
    } else {
        format!("{}…{}", &id[..6], &id[id.len().saturating_sub(4)..])
    }
}

/// OS-level Ctrl+V held? (bypasses egui-winit swallowing Key::V on paste)
fn raw_os_ctrl_v_down() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
        };
        const VK_V: i32 = 0x56;
        unsafe {
            let ctrl = (GetAsyncKeyState(VK_CONTROL as i32) as u16) & 0x8000 != 0
                || (GetAsyncKeyState(VK_LCONTROL as i32) as u16) & 0x8000 != 0
                || (GetAsyncKeyState(VK_RCONTROL as i32) as u16) & 0x8000 != 0;
            let v = (GetAsyncKeyState(VK_V) as u16) & 0x8000 != 0;
            ctrl && v
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

fn format_tokens_one(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}k", n / 1000)
    } else if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

fn format_tokens(used: u64, max: u64) -> String {
    format!(
        "{}/{}",
        format_tokens_one(used),
        format_tokens_one(max.max(1))
    )
}

fn compact_json_preview(v: &Value, max: usize) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            // Prefer common tool input fields
            for key in [
                "command",
                "path",
                "target_directory",
                "query",
                "pattern",
                "file",
            ] {
                if let Some(Value::String(s)) = map.get(key) {
                    return truncate_str(s, max);
                }
            }
            serde_json::to_string(v).unwrap_or_default()
        }
        _ => serde_json::to_string(v).unwrap_or_default(),
    };
    truncate_str(&s, max)
}

impl eframe::App for GrokApp {
    /// Avoid default black clear color bleeding as a thick edge between panels.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::BG();
        let a = c.a() as f32 / 255.0;
        [
            (c.r() as f32 / 255.0) * a,
            (c.g() as f32 / 255.0) * a,
            (c.b() as f32 / 255.0) * a,
            a,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count = self.frame_count.saturating_add(1);
        // Full-window stage fill first — kills 1–2px black seams between panels
        {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(screen, 0.0, theme::BG());
        }
        // After UI is up, connect agent once
        if self.pending_connect && self.frame_count >= 3 {
            self.pending_connect = false;
            self.connect_agent(ctx);
        }
        // Startup update check (deferred a few frames so first paint is snappy)
        if self.pending_update_check && self.frame_count == 5 {
            self.pending_update_check = false;
            self.start_update_check(ctx);
        }
        // Tray after a couple frames (event loop ready)
        if self.frame_count == 4
            || (self.config.show_tray && self.tray.is_none() && self.frame_count == 30)
        {
            self.ensure_tray(ctx);
        }
        // Drop tray if disabled in settings
        if !self.config.show_tray && self.tray.is_some() {
            self.tray = None;
            if self.window_hidden {
                self.show_main_window(ctx);
            }
        }
        // Re-apply title bar color a few times (HWND may not exist on first theme::apply).
        if self.frame_count == 2 || self.frame_count == 8 {
            crate::win_chrome::apply_titlebar_theme(self.config.dark_mode);
        }

        self.poll_tray(ctx);
        self.handle_close_to_tray(ctx);

        // MUST run before any TextEdit: intercept Ctrl+V image paste so the
        // input field cannot swallow it.
        self.poll_paste_early(ctx);

        // Light poll for "clipboard has image" chip — never while probing paste
        // (OpenClipboard is exclusive; concurrent probes cause spurious failures).
        if self.paste_probe_frames == 0 && self.frame_count % 15 == 0 {
            self.clipboard_image_ready = attachments::clipboard_has_image_hint();
        }

        self.poll_events(ctx);
        self.heal_tool_display_only();
        self.reconcile_idle_display();
        self.poll_install(ctx);
        self.poll_login(ctx);
        self.poll_image_generation(ctx);
        self.poll_readiness(ctx);
        self.poll_update_check(ctx);

        // Live session list rescan while streaming (titles update on disk)
        if self.store.busy() {
            let due = self
                .last_session_scan
                .map(|t| t.elapsed() > std::time::Duration::from_secs(3))
                .unwrap_or(true);
            if due {
                self.refresh_sessions();
            }
        }

        // Keep polling tray while hidden / idle so menu clicks work
        let need_tray_poll = self.tray.is_some();
        if self.store.busy()
            || self.store.is_connecting()
            || self.settings.installing
            || self.image_generating
            || self.onboarding_open
            || self.login_started
            || self.pending_connect
            || self.pending_update_check
            || self.update.checking
            || self.paste_probe_frames > 0
            || self.smooth_assistant.active
            || !self.smooth_assistant.is_caught_up()
            || need_tray_poll
            || self.window_hidden
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(if self.store.busy() {
                16
            } else if self.window_hidden {
                200
            } else {
                40
            }));
        }

        // Background fill
        let panel =
            egui::CentralPanel::default().frame(Frame::NONE.fill(theme::BG()).inner_margin(0.0));

        if self.sidebar_open {
            // No Frame stroke, no separator line, no multi-vline shadow.
            // Edge is only color difference (sidebar vs stage); optional 1px hairline.
            let side_resp = egui::SidePanel::left("sidebar")
                .exact_width(theme::SIDEBAR_WIDTH)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    Frame::NONE
                        .fill(theme::SIDEBAR())
                        .inner_margin(Margin::symmetric(10, 10))
                        .stroke(Stroke::NONE),
                )
                .show(ctx, |ui| {
                    self.ui_sidebar(ui, ctx);
                });
            // Ultra-soft 1px edge (not black bar). Paint on Foreground so it isn't scaled up.
            let r = side_resp.response.rect;
            let x = r.right() - 0.5;
            let stroke = if theme::is_dark() {
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 14))
            } else {
                // Very light — was stacking into a thick black strip
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 12))
            };
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("sidebar_edge"),
            ))
            .vline(x, r.y_range(), stroke);
        }

        panel.show(ctx, |ui| {
            self.ui_main(ui, ctx);
        });

        self.handle_settings_ui(ctx);
        self.ui_logs(ctx);
        self.ui_permission_modal(ctx);
        self.ui_update_modal(ctx);
        self.ui_update_badge(ctx);
        self.ui_rename_dialog(ctx);
        self.ui_archive_panel(ctx);
        self.ui_import_panel(ctx);
        self.ui_image_generation(ctx);
        self.ui_share(ctx);
        self.ui_image_preview(ctx);
        self.ui_onboarding(ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let _ = self.config.save();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.connect_generation.fetch_add(1, Ordering::SeqCst);
        self.stop_stream_pump();
        if let Some(c) = self.client.lock().take() {
            self.rt.block_on(async move {
                c.shutdown().await;
            });
        }
    }
}
