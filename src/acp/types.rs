use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Inbound line from agent stdout (request, response, or notification).
///
/// **Order matters** for `#[serde(untagged)]`:
/// - `JsonRpcResponse` has optional `result`/`error` with defaults, so it would
///   greedily match agent→client *requests* (which also have `id`) and drop the
///   `method` field. That left `fs/read_text_file` / `session/request_permission`
///   unanswered → tools hung forever (`xaiAcpChannelFailure: recv_failed`).
/// - Match Request first (requires `method` + `id`), then Response, then Notification.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum InboundMessage {
    /// Agent → client request (e.g. fs/read_text_file, session/request_permission)
    Request {
        jsonrpc: String,
        id: Value,
        method: String,
        #[serde(default)]
        params: Option<Value>,
    },
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

// --- UI-facing events ---

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Connected {
        agent_name: String,
        agent_version: String,
    },
    SessionCreated {
        session_id: String,
    },
    /// An existing session finished attaching to the current ACP process.
    /// Unlike `SessionCreated`, this must never replace the UI's selected id:
    /// a stale load may finish after the user selected another session.
    SessionLoaded {
        session_id: String,
    },
    ModeChanged {
        session_id: Option<String>,
        mode_id: String,
    },
    MessageChunk {
        text: String,
    },
    ThoughtChunk {
        text: String,
    },
    ToolCall {
        id: String,
        title: String,
        kind: String,
        status: String,
        raw_input: Option<Value>,
    },
    ToolCallUpdate {
        id: String,
        status: Option<String>,
        title: Option<String>,
        content_text: Option<String>,
    },
    Plan {
        entries: Vec<PlanEntry>,
    },
    PermissionRequest {
        request_id: Value,
        tool_call_id: String,
        title: String,
        options: Vec<PermissionOption>,
    },
    PromptFinished {
        stop_reason: String,
        /// Generation captured at send — ignore if a newer turn already started.
        turn_gen: u64,
    },
    /// Grok / ACP notify that the prompt turn ended (may arrive without waiting
    /// for the session/prompt JSON-RPC response to be matched).
    TurnCompleted {
        stop_reason: String,
    },
    Error {
        message: String,
        /// When set, only unlock UI if this still matches the active turn.
        turn_gen: Option<u64>,
    },
    /// Agent process exited. `pid` identifies which child so the UI can ignore
    /// stale exits from a previous reconnect.
    AgentExited {
        code: Option<i32>,
        pid: Option<u32>,
    },
    Log {
        message: String,
    },
    /// Context / token usage snapshot (from sessionUpdate or estimate).
    Usage {
        used: Option<u64>,
        max: Option<u64>,
        note: Option<String>,
    },
    /// Live model catalog from `_x.ai/models/update` (CLI 1.0+).
    ModelsUpdate {
        current_model_id: Option<String>,
        models: Vec<ModelCatalogEntry>,
    },
    /// Agent-reported prompt capabilities after `initialize`.
    AgentCapabilities {
        image: bool,
        agent_version: String,
    },
    /// Live slash / tool command list (`available_commands_update`).
    AvailableCommands {
        commands: Vec<AgentCommand>,
    },
    /// Structured Q&A from `x.ai/ask_user_question` (blocks the tool until answered).
    AskUserQuestion {
        request_id: Value,
        session_id: String,
        tool_call_id: String,
        questions: Vec<UserQuestion>,
        /// `default` | `plan`
        mode: String,
    },
    /// Product announcements from `_x.ai/announcements/update`.
    Announcements {
        items: Vec<AnnouncementItem>,
    },
}

/// One slash command advertised by the agent.
#[derive(Debug, Clone)]
pub struct AgentCommand {
    pub name: String,
    pub description: String,
    pub input_hint: Option<String>,
}

/// One question inside `x.ai/ask_user_question`.
#[derive(Debug, Clone)]
pub struct UserQuestion {
    pub question: String,
    pub options: Vec<UserQuestionOption>,
    pub multi_select: bool,
}

#[derive(Debug, Clone)]
pub struct UserQuestionOption {
    pub label: String,
    pub description: String,
    pub preview: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnnouncementItem {
    pub id: String,
    pub title: String,
    pub message: String,
    pub severity: String,
}

/// One model row from the live ACP catalog (or session/new `models`).
#[derive(Debug, Clone)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub name: String,
    pub context_window: Option<u64>,
    pub supports_reasoning_effort: bool,
    pub reasoning_efforts: Vec<String>,
    pub default_effort: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

// --- Display models ---

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

/// In-chat / composer image that can be thumbnailed and full-screen previewed.
#[derive(Debug, Clone)]
pub struct ChatImage {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    /// RGBA8 pixels for egui texture (may be downscaled for memory).
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum TimelineItem {
    UserMessage {
        id: String,
        text: String,
        /// Attached images (pixels kept for UI preview).
        attachments: Vec<ChatImage>,
    },
    AssistantMessage {
        id: String,
        text: String,
        streaming: bool,
    },
    Thought {
        id: String,
        text: String,
    },
    Tool {
        id: String,
        title: String,
        kind: String,
        status: String,
        detail: String,
    },
    Plan {
        id: String,
        entries: Vec<PlanEntry>,
    },
    Status {
        id: String,
        text: String,
    },
}
