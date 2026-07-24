use super::types::*;
use crate::config::{resolve_grok_binary, AppConfig};
use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};

/// JSON-RPC ACP client that wraps `grok agent stdio`.
pub struct AcpClient {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    next_id: AtomicU64,
    /// Async mutex so concurrent writers (UI cancel + fs response) never race
    /// on `take()` of ChildStdin (that used to return "stdin closed" mid-tool).
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    alive: AtomicBool,
    /// Guard against double AgentExited (stdout EOF + wait).
    exited_notified: AtomicBool,
    session_id: Mutex<Option<String>>,
    child_id: Mutex<Option<u32>>,
    /// When true, answer session/request_permission immediately (ACP host-side).
    /// Must not wait for UI — otherwise tools freeze mid-turn.
    always_approve: AtomicBool,
    /// True while a session/prompt RPC is outstanding. Concurrent prompts corrupt turns.
    prompt_inflight: AtomicBool,
}

impl AcpClient {
    /// Spawn `grok agent … stdio` and run the I/O loop.
    pub async fn start(
        config: &AppConfig,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
    ) -> Result<Self> {
        let binary = resolve_grok_binary(&config.grok_path)?;
        let _ = event_tx.send(AgentEvent::Log {
            message: format!("启动 agent: {}", binary.display()),
        });

        let mut args: Vec<String> = vec!["agent".into()];
        if !config.model.trim().is_empty() {
            args.push("--model".into());
            args.push(config.model.trim().into());
        }
        let effort = crate::config::normalize_effort(&config.effort);
        args.push("--reasoning-effort".into());
        args.push(effort.into());
        if config.always_approve {
            args.push("--always-approve".into());
        }
        for a in &config.extra_agent_args {
            args.push(a.clone());
        }
        args.push("stdio".into());
        let _ = event_tx.send(AgentEvent::Log {
            message: format!(
                "agent args: model={} effort={} always_approve={}",
                config.model, effort, config.always_approve
            ),
        });

        let mut cmd = Command::new(&binary);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Hide console window for the child agent on Windows GUI builds.
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawn {}", binary.display()))?;

        let stdin = child.stdin.take().context("agent stdin")?;
        let stdout = child.stdout.take().context("agent stdout")?;
        let stderr = child.stderr.take().context("agent stderr")?;
        let pid = child.id();

        let inner = Arc::new(ClientInner {
            next_id: AtomicU64::new(1),
            stdin: tokio::sync::Mutex::new(Some(stdin)),
            pending: Mutex::new(HashMap::new()),
            event_tx: event_tx.clone(),
            alive: AtomicBool::new(true),
            exited_notified: AtomicBool::new(false),
            session_id: Mutex::new(None),
            child_id: Mutex::new(pid),
            always_approve: AtomicBool::new(config.always_approve),
            prompt_inflight: AtomicBool::new(false),
        });

        // stdout reader — do NOT emit AgentExited here (wait_child owns that),
        // otherwise reconnect races fire two exits and the UI may drop the new client.
        {
            let inner = inner.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Err(e) = inner.handle_line(&line).await {
                        tracing::debug!("handle line error: {e} | {line}");
                    }
                }
                inner.alive.store(false, Ordering::SeqCst);
            });
        }

        // stderr → log
        {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    tracing::info!("grok stderr: {line}");
                    let _ = tx.send(AgentEvent::Log {
                        message: format!("[cli] {line}"),
                    });
                }
            });
        }

        // wait child — single AgentExited notification with pid
        {
            let inner = inner.clone();
            let child_pid = pid;
            tokio::spawn(async move {
                let code = wait_child(child).await;
                inner.alive.store(false, Ordering::SeqCst);
                if !inner.exited_notified.swap(true, Ordering::SeqCst) {
                    let _ = inner.event_tx.send(AgentEvent::AgentExited {
                        code,
                        pid: child_pid,
                    });
                }
            });
        }

        let client = Self { inner };
        // Only initialize — do NOT session/new here. Creating a session on every
        // connect spams empty ~/.grok/sessions entries. Session is created lazily
        // on first prompt or explicit「新对话」.
        client.initialize().await?;
        Ok(client)
    }

    pub fn is_alive(&self) -> bool {
        self.inner.alive.load(Ordering::SeqCst)
    }

    pub fn child_pid(&self) -> Option<u32> {
        *self.inner.child_id.lock()
    }

    pub fn session_id(&self) -> Option<String> {
        self.inner.session_id.lock().clone()
    }

    /// Drop the in-memory session id so the next prompt/new creates a fresh one.
    pub fn clear_session(&self) {
        *self.inner.session_id.lock() = None;
    }

    /// Ensure an active ACP session exists (lazy `session/new`).
    pub async fn ensure_session(&self, cwd: &str) -> Result<String> {
        if let Some(id) = self.session_id() {
            return Ok(id);
        }
        self.new_session(cwd).await
    }

    async fn initialize(&self) -> Result<()> {
        let params = json!({
            "protocolVersion": 1,
            // Advertise only what we implement — claiming terminal without handlers
            // freezes the agent waiting for createTerminal responses.
            "clientCapabilities": {
                "fs": { "readTextFile": true, "writeTextFile": true },
                "terminal": false
            },
            "clientInfo": {
                "name": "grok-app",
                "title": "Grok Desktop",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let result = self.request("initialize", params).await?;
        let agent_name = result
            .pointer("/agentInfo/name")
            .and_then(|v| v.as_str())
            .unwrap_or("grok")
            .to_string();
        let agent_version = result
            .pointer("/agentInfo/version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let _ = self.inner.event_tx.send(AgentEvent::Connected {
            agent_name,
            agent_version,
        });

        // Note: do not send a generic `authenticated` notify — current grok
        // agent logs "Method not found" for it. Auth is already via ~/.grok/auth.json.
        Ok(())
    }

    pub async fn new_session(&self, cwd: &str) -> Result<String> {
        let params = json!({
            "cwd": cwd,
            "mcpServers": []
        });
        let result = self.request("session/new", params).await?;
        let session_id = result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("session/new missing sessionId: {result}"))?
            .to_string();
        *self.inner.session_id.lock() = Some(session_id.clone());
        let _ = self.inner.event_tx.send(AgentEvent::SessionCreated {
            session_id: session_id.clone(),
        });
        Ok(session_id)
    }

    /// Resume a session stored under ~/.grok/sessions (ACP `session/load`).
    pub async fn load_session(&self, session_id: &str, cwd: &str) -> Result<String> {
        let params = json!({
            "sessionId": session_id,
            "cwd": cwd,
            "mcpServers": []
        });
        let result = self.request("session/load", params).await?;
        let sid = result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or(session_id)
            .to_string();
        *self.inner.session_id.lock() = Some(sid.clone());
        let _ = self.inner.event_tx.send(AgentEvent::SessionCreated {
            session_id: sid.clone(),
        });
        Ok(sid)
    }

    /// Send a user prompt; streams arrive as AgentEvents; resolves when turn ends.
    pub async fn prompt(&self, text: &str, cwd: &str) -> Result<String> {
        self.prompt_text(text, cwd).await
    }

    /// Send multimodal prompt blocks (text / image per ACP ContentBlock).
    /// Creates a session on first send if none is active.
    /// `history_bootstrap` is optional host-only context (not shown in UI).
    pub fn is_prompt_inflight(&self) -> bool {
        self.inner.prompt_inflight.load(Ordering::SeqCst)
    }

    pub async fn prompt_blocks(
        &self,
        mut blocks: Vec<serde_json::Value>,
        cwd: &str,
        history_bootstrap: Option<String>,
    ) -> Result<String> {
        if blocks.is_empty() {
            return Err(anyhow!("empty prompt"));
        }
        if let Some(boot) = history_bootstrap {
            if !boot.is_empty() {
                blocks.insert(0, json!({ "type": "text", "text": boot }));
            }
        }

        // ACP allows only one prompt turn at a time. If a previous RPC is still
        // open (UI force-unlocked without cancel, or hang), cancel and wait.
        if self
            .inner
            .prompt_inflight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            let _ = self.event_tx().send(AgentEvent::Log {
                message: "上一轮 session/prompt 仍在进行，先 cancel 再发".into(),
            });
            let _ = self.cancel().await;
            // Give the agent a moment to return cancelled stopReason
            for _ in 0..40 {
                if !self.inner.prompt_inflight.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            self.inner.prompt_inflight.store(true, Ordering::SeqCst);
        }

        let session_id = self.ensure_session(cwd).await?;
        let params = crate::acp::parse::build_prompt_params(&session_id, &blocks);
        let result = self.request("session/prompt", params).await;
        self.inner.prompt_inflight.store(false, Ordering::SeqCst);
        let result = result?;
        let stop = result
            .get("stopReason")
            .or_else(|| result.get("stop_reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("end_turn")
            .to_string();
        Ok(stop)
    }

    fn event_tx(&self) -> mpsc::UnboundedSender<AgentEvent> {
        self.inner.event_tx.clone()
    }

    /// Prompt with plain text only.
    pub async fn prompt_text(&self, text: &str, cwd: &str) -> Result<String> {
        self.prompt_blocks(vec![json!({ "type": "text", "text": text })], cwd, None)
            .await
    }

    pub async fn cancel(&self) -> Result<()> {
        let session_id = match self.inner.session_id.lock().clone() {
            Some(s) => s,
            None => {
                self.inner.prompt_inflight.store(false, Ordering::SeqCst);
                return Ok(());
            }
        };
        let res = self
            .notify("session/cancel", json!({ "sessionId": session_id }))
            .await;
        // Do not clear prompt_inflight here — wait until session/prompt RPC returns
        // (cancelled). Clearing early allows a second prompt while agent still winds down.
        res
    }

    pub async fn respond_permission(&self, request_id: Value, option_id: &str) -> Result<()> {
        let result = json!({
            "outcome": {
                "outcome": "selected",
                "optionId": option_id
            }
        });
        self.write_response(request_id, result).await
    }

    pub async fn cancel_permission(&self, request_id: Value) -> Result<()> {
        let result = json!({
            "outcome": { "outcome": "cancelled" }
        });
        self.write_response(request_id, result).await
    }

    pub async fn shutdown(&self) {
        let _ = self.cancel().await;
        // Drop stdin to signal EOF
        *self.inner.stdin.lock().await = None;
        self.inner.alive.store(false, Ordering::SeqCst);
        // Best-effort kill
        if let Some(pid) = *self.inner.child_id.lock() {
            kill_pid(pid);
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().insert(id, tx);

        let msg = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params: Some(params),
        };
        self.write_json(&msg).await?;

        // ACP turns can be long (many tools). Only a hard safety ceiling —
        // never cancel early; that is session/cancel's job (user-initiated).
        let timeout_s = if method == "session/prompt" {
            30 * 60 // 30 minutes
        } else if method == "session/load" || method == "session/new" {
            120
        } else if method.contains("rewind") {
            // Large sessions: file restore + history rewrite can exceed 60s
            300
        } else {
            60
        };
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_s), rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(anyhow!("request channel closed for {method}")),
            Err(_) => {
                self.inner.pending.lock().remove(&id);
                Err(anyhow!("timeout waiting for {method} ({timeout_s}s)"))
            }
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.write_value(&msg).await
    }

    async fn write_response(&self, id: Value, result: Value) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });
        self.write_value(&msg).await
    }

    async fn write_json<T: serde::Serialize>(&self, value: &T) -> Result<()> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        self.inner.write_bytes(line.as_bytes()).await
    }

    async fn write_value(&self, value: &Value) -> Result<()> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        self.inner.write_bytes(line.as_bytes()).await
    }
}

impl ClientInner {
    async fn handle_line(&self, line: &str) -> Result<()> {
        let msg: InboundMessage = serde_json::from_str(line)
            .with_context(|| format!("parse json-rpc: {}", trunc(line, 200)))?;

        match msg {
            InboundMessage::Response(resp) => {
                let id = match &resp.id {
                    Value::Number(n) => n.as_u64(),
                    Value::String(s) => s.parse().ok(),
                    _ => None,
                };
                if let Some(id) = id {
                    if let Some(tx) = self.pending.lock().remove(&id) {
                        if let Some(err) = resp.error {
                            let _ = tx.send(Err(anyhow!("RPC {}: {}", err.code, err.message)));
                        } else {
                            let _ = tx.send(Ok(resp.result.unwrap_or(Value::Null)));
                        }
                    }
                }
            }
            InboundMessage::Request {
                id, method, params, ..
            } => {
                self.handle_agent_request(id, &method, params.unwrap_or(Value::Null))
                    .await?;
            }
            InboundMessage::Notification(n) => {
                self.handle_notification(&n.method, n.params.unwrap_or(Value::Null))?;
            }
        }
        Ok(())
    }

    async fn handle_agent_request(&self, id: Value, method: &str, params: Value) -> Result<()> {
        match method {
            "session/request_permission" => {
                let tool = params.get("toolCall").cloned().unwrap_or(Value::Null);
                let tool_call_id = tool
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = tool
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Tool permission")
                    .to_string();
                let options: Vec<PermissionOption> = params
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| {
                                Some(PermissionOption {
                                    option_id: o.get("optionId")?.as_str()?.to_string(),
                                    name: o
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Option")
                                        .to_string(),
                                    kind: o
                                        .get("kind")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Auto-approve at the RPC layer (official ACP: client MAY auto-allow).
                // Never park the agent waiting for a UI frame — that freezes tools mid-turn.
                if self.always_approve.load(Ordering::Relaxed) {
                    let opt = pick_allow_option_id(&options).unwrap_or_else(|| {
                        options
                            .first()
                            .map(|o| o.option_id.clone())
                            .unwrap_or_else(|| "allow-once".into())
                    });
                    let _ = self
                        .write_raw(&json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "outcome": {
                                    "outcome": "selected",
                                    "optionId": opt
                                }
                            }
                        }))
                        .await;
                    let _ = self.event_tx.send(AgentEvent::Log {
                        message: format!("自动批准权限: {title} ({tool_call_id}) → {opt}"),
                    });
                } else {
                    let _ = self.event_tx.send(AgentEvent::PermissionRequest {
                        request_id: id,
                        tool_call_id,
                        title,
                        options,
                    });
                }
            }
            // ACP file ops — must answer or the agent turn freezes mid-tool.
            // Also accept camelCase aliases some agents emit.
            "fs/read_text_file" | "fs/readTextFile" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = params
                    .get("line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let limit = params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                // Offload blocking IO so the stdout reader keeps draining.
                let path_for_io = path.clone();
                let read_res = tokio::task::spawn_blocking(move || {
                    read_text_file_host(&path_for_io, line, limit)
                })
                .await
                .unwrap_or_else(|e| Err(format!("read join: {e}")));
                match read_res {
                    Ok(content) => {
                        if let Err(e) = self
                            .write_raw(&json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "content": content }
                            }))
                            .await
                        {
                            tracing::error!("fs/read_text_file reply failed: {e:#}");
                            let _ = self.event_tx.send(AgentEvent::Log {
                                message: format!("fs/read_text_file REPLY FAIL {path}: {e:#}"),
                            });
                        } else {
                            let _ = self.event_tx.send(AgentEvent::Log {
                                message: format!("fs/read_text_file ok {}", trunc(&path, 80)),
                            });
                        }
                    }
                    Err(e) => {
                        if let Err(we) = self
                            .write_raw(&json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32000,
                                    "message": format!("read failed: {e}")
                                }
                            }))
                            .await
                        {
                            tracing::error!("fs/read_text_file error reply failed: {we:#}");
                        }
                        let _ = self.event_tx.send(AgentEvent::Log {
                            message: format!("fs/read_text_file ERR {}: {e}", trunc(&path, 80)),
                        });
                    }
                }
            }
            "fs/write_text_file" | "fs/writeTextFile" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let path_w = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    if let Some(parent) = std::path::Path::new(&path_w).parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                    std::fs::write(&path_w, content)
                })
                .await
                .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));
                match result {
                    Ok(()) => {
                        if let Err(e) = self
                            .write_raw(&json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {}
                            }))
                            .await
                        {
                            tracing::error!("fs/write_text_file reply failed: {e:#}");
                        }
                        let _ = self.event_tx.send(AgentEvent::Log {
                            message: format!("fs/write_text_file {}", trunc(&path, 80)),
                        });
                    }
                    Err(e) => {
                        let _ = self
                            .write_raw(&json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32000,
                                    "message": format!("write failed: {e}")
                                }
                            }))
                            .await;
                    }
                }
            }
            other => {
                tracing::warn!("unhandled agent request: {other}");
                // Always reply — never leave the agent blocked on a missing method.
                let msg = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not implemented by desktop client: {other}")
                    }
                });
                if let Err(e) = self.write_raw(&msg).await {
                    tracing::error!("unhandled method reply failed: {e:#}");
                }
            }
        }
        Ok(())
    }

    async fn write_raw(&self, value: &Value) -> Result<()> {
        let mut line = serde_json::to_string(value)?;
        line.push('\n');
        self.write_bytes(line.as_bytes()).await
    }
}

impl ClientInner {
    /// Serialize all stdin writes through an async mutex (no take/race).
    async fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut guard = self.stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| anyhow!("agent stdin closed"))?;
        stdin.write_all(bytes).await.context("write stdin")?;
        stdin.flush().await.context("flush stdin")?;
        Ok(())
    }

    fn handle_notification(&self, method: &str, params: Value) -> Result<()> {
        // Official ACP + Grok variants: session/update, _x.ai/session/update, x.ai/...
        let is_session_update = method == "session/update"
            || method.ends_with("/session/update")
            || method.contains("session/update");
        if is_session_update {
            let update = params.get("update").cloned().unwrap_or(Value::Null);
            let mut events = crate::acp::parse::session_update_to_events(&update);
            // x.ai background task lifecycle → force tool terminal state
            if events.is_empty() {
                let kind = update
                    .get("sessionUpdate")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if kind == "task_completed" || kind == "task_backgrounded" {
                    let tool_id = update
                        .get("tool_call_id")
                        .or_else(|| update.get("toolCallId"))
                        .or_else(|| {
                            update
                                .pointer("/task_snapshot/task_id")
                                .or_else(|| update.pointer("/taskSnapshot/taskId"))
                        })
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !tool_id.is_empty() {
                        let status = if kind == "task_completed" {
                            "completed".to_string()
                        } else {
                            // backgrounded: still treat as no longer blocking the UI row
                            "completed".to_string()
                        };
                        events.push(AgentEvent::ToolCallUpdate {
                            id: tool_id,
                            status: Some(status),
                            title: None,
                            content_text: None,
                        });
                    }
                } else if !kind.is_empty() {
                    tracing::debug!("unhandled sessionUpdate: {kind}");
                }
            }
            for ev in events {
                let _ = self.event_tx.send(ev);
            }
            return Ok(());
        }
        match method {
            m if m.starts_with("x.ai/") || m.starts_with("_x.ai/") => {
                tracing::debug!("x.ai notification: {m}");
            }
            other => {
                tracing::debug!("notification: {other}");
            }
        }
        Ok(())
    }
}

fn trunc(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

fn pick_allow_option_id(options: &[PermissionOption]) -> Option<String> {
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
    options
        .iter()
        .find(|o| {
            let blob = format!("{} {} {}", o.option_id, o.name, o.kind).to_ascii_lowercase();
            blob.contains("allow") || blob.contains("approve")
        })
        .map(|o| o.option_id.clone())
}

/// Optional 1-based line/limit slice for ACP fs/read_text_file.
fn slice_text_lines(content: &str, line: Option<usize>, limit: Option<usize>) -> String {
    let Some(start) = line.filter(|&n| n >= 1) else {
        return content.to_string();
    };
    let lines: Vec<&str> = content.lines().collect();
    let from = start.saturating_sub(1).min(lines.len());
    let take = limit.unwrap_or(lines.len().saturating_sub(from));
    lines[from..]
        .iter()
        .take(take)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Host-side fs/read_text_file: binary-safe, size-capped, never hang the agent.
fn read_text_file_host(
    path: &str,
    line: Option<usize>,
    limit: Option<usize>,
) -> std::result::Result<String, String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file".into());
    }
    const MAX_BYTES: u64 = 2 * 1024 * 1024;
    let len = meta.len();
    // Detect obvious binary by extension first
    let lower = path.to_ascii_lowercase();
    let binary_ext = [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".ico", ".pdf", ".zip", ".exe", ".dll",
        ".so", ".dylib", ".wasm", ".mp4", ".mp3", ".wav", ".7z", ".rar", ".gz", ".tar", ".woff",
        ".woff2", ".ttf", ".otf",
    ];
    if binary_ext.iter().any(|e| lower.ends_with(e)) {
        return Ok(format!(
            "[binary file: {path} ({len} bytes) — not text; use vision/image blocks or a binary-aware tool]"
        ));
    }
    // Cap read size
    let to_read = len.min(MAX_BYTES) as usize;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    use std::io::Read;
    let mut buf = vec![0u8; to_read];
    let n = f.read(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(n);
    // If high ratio of NUL / non-text, treat as binary
    let nul = buf.iter().filter(|&&b| b == 0).count();
    if n > 0 && nul * 20 > n {
        return Ok(format!(
            "[binary file: {path} ({len} bytes) — content is not valid text]"
        ));
    }
    let mut content = String::from_utf8_lossy(&buf).into_owned();
    if len > MAX_BYTES {
        content.push_str(&format!(
            "\n…[truncated: file is {len} bytes, showing first {MAX_BYTES}]"
        ));
    }
    Ok(slice_text_lines(&content, line, limit))
}

async fn wait_child(mut child: Child) -> Option<i32> {
    match child.wait().await {
        Ok(status) => status.code(),
        Err(_) => None,
    }
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        // Hidden — avoid console flash on disconnect / reconnect.
        let _ = crate::spawn_util::command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}
