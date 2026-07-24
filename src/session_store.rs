//! Single source of truth for host ↔ agent runtime state.
//!
//! UI (topbar / sidebar / message rail) must **read** projections from here.
//! Only command handlers and the ACP event loop **write**.
//!
//! Timeline text and smooth-stream drip stay in the app (document / view state).

use crate::acp::PermissionOption;
use crate::ui::widgets::SessionActivity;
use serde_json::Value;
use std::time::Instant;

// ── Phases ──────────────────────────────────────────────────────────────

/// Connection to the agent process (stdio child).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnPhase {
    #[default]
    Disconnected,
    Connecting,
    Ready,
}

impl ConnPhase {
    pub fn label(self) -> &'static str {
        let s = crate::i18n::t();
        match self {
            Self::Disconnected => s.agent_disconnected,
            Self::Connecting => s.connecting,
            Self::Ready => s.ready,
        }
    }
}

/// In-turn activity (message column status rail + topbar pill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurnPhase {
    #[default]
    Idle,
    Thinking,
    Tool,
    Generating,
    Permission,
}

impl TurnPhase {
    pub fn label(self) -> &'static str {
        let s = crate::i18n::t();
        match self {
            Self::Idle => s.ready,
            Self::Thinking => s.phase_thinking,
            Self::Tool => s.tool_default,
            Self::Generating => s.generating,
            Self::Permission => s.act_permission,
        }
    }

    pub fn is_active(self) -> bool {
        !matches!(self, Self::Idle)
    }
}

/// Coarse host label (slash /status, legacy FSM-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPhase {
    Idle,
    Connecting,
    Ready,
    Streaming,
    AwaitingPermission,
    Disconnected,
}

impl HostPhase {
    pub fn label(self) -> &'static str {
        let s = crate::i18n::t();
        match self {
            Self::Idle => s.ready,
            Self::Connecting => s.connecting,
            Self::Ready => s.ready,
            Self::Streaming => s.generating,
            Self::AwaitingPermission => s.act_permission,
            Self::Disconnected => s.agent_disconnected,
        }
    }
}

// ── Permission ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub request_id: Value,
    pub tool_call_id: String,
    pub title: String,
    pub options: Vec<PermissionOption>,
}

// ── Store ───────────────────────────────────────────────────────────────

/// Agent + turn runtime. One process ↔ one open session (host), with sticky
/// `busy_session_id` so the sidebar can show activity without that row selected.
#[derive(Debug, Default)]
pub struct SessionStore {
    conn: ConnPhase,
    /// Currently bound agent session id (UI open / last created).
    session_id: Option<String>,
    /// Session that owns the in-flight turn (sticky for sidebar).
    busy_session_id: Option<String>,
    busy: bool,
    busy_since: Option<Instant>,
    last_activity: Option<Instant>,
    /// Monotonic turn id — late PromptFinished from an old RPC cannot kill a new turn.
    turn_gen: u64,
    live_tool: Option<(String, String)>,
    open_assistant_id: Option<String>,
    open_thought_id: Option<String>,
    pending_permission: Option<PendingPermission>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Reads ─────────────────────────────────────────────────────────

    pub fn conn(&self) -> ConnPhase {
        self.conn
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.conn, ConnPhase::Ready)
    }

    pub fn is_connecting(&self) -> bool {
        matches!(self.conn, ConnPhase::Connecting)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn session_id_owned(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub fn set_session_id(&mut self, id: Option<String>) {
        self.session_id = id;
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn busy_session_id(&self) -> Option<&str> {
        self.busy_session_id.as_deref()
    }

    pub fn busy_session_id_owned(&self) -> Option<String> {
        self.busy_session_id.clone()
    }

    pub fn busy_since(&self) -> Option<Instant> {
        self.busy_since
    }

    pub fn last_activity(&self) -> Option<Instant> {
        self.last_activity
    }

    pub fn turn_gen(&self) -> u64 {
        self.turn_gen
    }

    /// Whether `gen` is still the active in-flight turn.
    pub fn is_turn_gen(&self, gen: u64) -> bool {
        self.busy && self.turn_gen == gen
    }

    pub fn live_tool(&self) -> Option<&(String, String)> {
        self.live_tool.as_ref()
    }

    pub fn live_tool_owned(&self) -> Option<(String, String)> {
        self.live_tool.clone()
    }

    pub fn open_assistant_id(&self) -> Option<&str> {
        self.open_assistant_id.as_deref()
    }

    pub fn open_assistant_id_owned(&self) -> Option<String> {
        self.open_assistant_id.clone()
    }

    pub fn open_thought_id(&self) -> Option<&str> {
        self.open_thought_id.as_deref()
    }

    pub fn open_thought_id_owned(&self) -> Option<String> {
        self.open_thought_id.clone()
    }

    pub fn pending_permission(&self) -> Option<&PendingPermission> {
        self.pending_permission.as_ref()
    }

    pub fn take_pending_permission(&mut self) -> Option<PendingPermission> {
        self.pending_permission.take()
    }

    /// Fine-grained turn phase for status rail / topbar.
    pub fn turn_phase(&self) -> TurnPhase {
        if !self.busy && self.pending_permission.is_none() {
            return TurnPhase::Idle;
        }
        if self.pending_permission.is_some() {
            return TurnPhase::Permission;
        }
        if let Some((_, st)) = &self.live_tool {
            let s = st.to_ascii_lowercase();
            if s == "in_progress" || s == "running" || s == "pending" {
                return TurnPhase::Tool;
            }
        }
        if self.open_thought_id.is_some() && self.open_assistant_id.is_none() {
            return TurnPhase::Thinking;
        }
        if self.open_assistant_id.is_some() {
            return TurnPhase::Generating;
        }
        // Busy but no open stream / tool chip → still in turn (tools between segments)
        if self.busy {
            return TurnPhase::Generating;
        }
        TurnPhase::Idle
    }

    /// Coarse host phase (replaces SessionFsm.state()).
    pub fn host_phase(&self) -> HostPhase {
        match self.conn {
            ConnPhase::Disconnected => HostPhase::Disconnected,
            ConnPhase::Connecting => HostPhase::Connecting,
            ConnPhase::Ready => {
                if self.pending_permission.is_some() {
                    HostPhase::AwaitingPermission
                } else if self.busy {
                    HostPhase::Streaming
                } else {
                    HostPhase::Ready
                }
            }
        }
    }

    /// Sidebar row activity — works without the session being selected.
    pub fn sidebar_activity(
        &self,
        session_id: &str,
        selected: bool,
        cli_live: bool,
    ) -> SessionActivity {
        let is_our_busy = self.busy && self.busy_session_id.as_deref() == Some(session_id);
        if is_our_busy {
            return match self.turn_phase() {
                TurnPhase::Permission => SessionActivity::Permission,
                TurnPhase::Tool => SessionActivity::Tool,
                TurnPhase::Thinking | TurnPhase::Generating => SessionActivity::Generating,
                TurnPhase::Idle => SessionActivity::Generating,
            };
        }
        if cli_live && !selected && self.busy_session_id.as_deref() != Some(session_id) {
            return SessionActivity::Generating;
        }
        if selected && self.is_connecting() {
            return SessionActivity::Connecting;
        }
        if selected {
            SessionActivity::Current
        } else {
            SessionActivity::Idle
        }
    }

    // ── Connection writes ─────────────────────────────────────────────

    pub fn begin_connect(&mut self) {
        self.conn = ConnPhase::Connecting;
    }

    pub fn handshake_ok(&mut self) {
        self.conn = ConnPhase::Ready;
    }

    /// Process gone / user disconnect. Clears turn so UI never stays locked.
    pub fn disconnect(&mut self) {
        self.conn = ConnPhase::Disconnected;
        self.clear_turn();
    }

    pub fn mark_connect_failed(&mut self) {
        self.conn = ConnPhase::Disconnected;
        self.clear_turn();
    }

    // ── Turn writes ───────────────────────────────────────────────────

    /// User (or host) started a prompt. Returns the generation for this turn.
    pub fn begin_turn(&mut self) -> u64 {
        let now = Instant::now();
        self.turn_gen = self.turn_gen.wrapping_add(1);
        self.busy = true;
        self.busy_session_id = self.session_id.clone();
        self.busy_since = Some(now);
        self.last_activity = Some(now);
        self.open_assistant_id = None;
        self.open_thought_id = None;
        self.live_tool = None;
        self.pending_permission = None;
        self.turn_gen
    }

    pub fn end_turn(&mut self) {
        self.clear_turn();
    }

    /// End only if `gen` is still the current busy turn (stale RPC-safe).
    pub fn end_turn_if_gen(&mut self, gen: u64) -> bool {
        if self.turn_gen != gen {
            return false;
        }
        self.clear_turn();
        true
    }

    /// Always free the UI — wedged agent / force stop. Bumps gen so late RPCs are ignored.
    pub fn force_unlock(&mut self) {
        self.turn_gen = self.turn_gen.wrapping_add(1);
        self.clear_turn();
    }

    fn clear_turn(&mut self) {
        self.busy = false;
        self.busy_session_id = None;
        self.busy_since = None;
        self.last_activity = None;
        self.live_tool = None;
        self.pending_permission = None;
        self.open_assistant_id = None;
        self.open_thought_id = None;
    }

    pub fn touch_activity(&mut self) {
        if self.busy {
            self.last_activity = Some(Instant::now());
        }
    }

    /// Switching session view: drop open stream cursors, keep busy sticky.
    pub fn on_switch_session_view(&mut self, session_id: String) {
        self.session_id = Some(session_id);
        self.open_assistant_id = None;
        self.open_thought_id = None;
        // live_tool stays if this is the busy session; clear if viewing another
        if self.busy_session_id.as_ref() != self.session_id.as_ref() {
            // Don't clear live_tool for sidebar of busy session — keep global
            // for topbar while user peeks another chat. Acceptable tradeoff.
        }
    }

    pub fn on_new_chat(&mut self) {
        self.session_id = None;
        // Ending local view — if mid-turn, still unlock host (new session)
        self.clear_turn();
    }

    pub fn clear_stream_cursors(&mut self) {
        self.open_assistant_id = None;
        self.open_thought_id = None;
    }

    pub fn set_open_assistant(&mut self, id: Option<String>) {
        self.open_assistant_id = id;
    }

    pub fn set_open_thought(&mut self, id: Option<String>) {
        self.open_thought_id = id;
    }

    pub fn note_message_chunk(&mut self) {
        self.live_tool = None;
        // Never auto begin_turn from stream noise — would steal turn_gen.
        if self.busy {
            self.touch_activity();
        }
    }

    pub fn note_thought_chunk(&mut self) {
        if self.busy {
            self.touch_activity();
        }
    }

    pub fn note_tool_call(&mut self, title: &str, status: &str) {
        self.live_tool = Some((title.to_string(), status.to_string()));
        self.touch_activity();
    }

    pub fn note_tool_status(&mut self, title: &str, status: &str) {
        let st = status.to_ascii_lowercase();
        if matches!(
            st.as_str(),
            "completed"
                | "complete"
                | "success"
                | "succeeded"
                | "done"
                | "failed"
                | "error"
                | "cancelled"
                | "canceled"
        ) {
            self.live_tool = None;
        } else if matches!(
            st.as_str(),
            "in_progress" | "running" | "pending" | "queued" | "active"
        ) {
            // Don't resurrect live chip if we already cleared after a terminal status
            // in this frame — caller handles monotonicity for timeline rows.
            self.live_tool = Some((title.to_string(), status.to_string()));
        }
        self.touch_activity();
    }

    pub fn set_permission(&mut self, pending: PendingPermission) {
        self.pending_permission = Some(pending);
        self.touch_activity();
    }

    pub fn clear_permission(&mut self) {
        self.pending_permission = None;
    }

    pub fn clear_live_tool(&mut self) {
        self.live_tool = None;
    }

    /// Soft error during connect (keep Ready if already connected).
    pub fn abort_connecting(&mut self) {
        if matches!(self.conn, ConnPhase::Connecting) {
            self.conn = ConnPhase::Disconnected;
        }
        self.clear_turn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_phase_priority() {
        let mut s = SessionStore::new();
        s.handshake_ok();
        assert_eq!(s.turn_phase(), TurnPhase::Idle);

        s.set_session_id(Some("abc".into()));
        let g = s.begin_turn();
        assert!(g >= 1);
        assert_eq!(s.turn_phase(), TurnPhase::Generating);

        s.note_tool_call("read", "in_progress");
        assert_eq!(s.turn_phase(), TurnPhase::Tool);

        s.set_permission(PendingPermission {
            request_id: Value::Null,
            tool_call_id: "t1".into(),
            title: "read".into(),
            options: vec![],
        });
        assert_eq!(s.turn_phase(), TurnPhase::Permission);

        s.end_turn();
        assert_eq!(s.turn_phase(), TurnPhase::Idle);
        assert!(!s.busy());
    }

    #[test]
    fn stale_turn_gen_ignored() {
        let mut s = SessionStore::new();
        s.handshake_ok();
        s.set_session_id(Some("s1".into()));
        let g1 = s.begin_turn();
        s.force_unlock();
        let g2 = s.begin_turn();
        assert_ne!(g1, g2);
        assert!(!s.end_turn_if_gen(g1));
        assert!(s.busy());
        assert!(s.end_turn_if_gen(g2));
        assert!(!s.busy());
    }

    #[test]
    fn sidebar_sticky_busy() {
        let mut s = SessionStore::new();
        s.handshake_ok();
        s.set_session_id(Some("s1".into()));
        s.begin_turn();
        assert_eq!(
            s.sidebar_activity("s1", false, false),
            SessionActivity::Generating
        );
        assert_eq!(
            s.sidebar_activity("s2", true, false),
            SessionActivity::Current
        );
    }
}
