//! Pure parsers for ACP `session/update` payloads (testable without a live agent).

use super::types::{AgentEvent, PlanEntry};
use serde_json::Value;

/// Map one `session/update` `update` object into zero or more UI events.
pub fn session_update_to_events(update: &Value) -> Vec<AgentEvent> {
    let kind = update
        .get("sessionUpdate")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match kind {
        "agent_message_chunk" | "agent_message" | "message_chunk" => {
            let text = extract_text_content(update);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::MessageChunk { text }]
            }
        }
        "agent_thought_chunk" | "agent_thought" | "thought_chunk" => {
            let text = extract_text_content(update);
            if text.is_empty() {
                Vec::new()
            } else {
                vec![AgentEvent::ThoughtChunk { text }]
            }
        }
        "tool_call" => {
            let id = update
                .get("toolCallId")
                .or_else(|| update.get("tool_call_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = update
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string();
            let kind = update
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("other")
                .to_string();
            let status = normalize_tool_status(
                update
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending"),
            );
            let raw_input = update
                .get("rawInput")
                .or_else(|| update.get("raw_input"))
                .cloned();
            vec![AgentEvent::ToolCall {
                id,
                title,
                kind,
                status,
                raw_input,
            }]
        }
        "tool_call_update" => {
            let id = update
                .get("toolCallId")
                .or_else(|| update.get("tool_call_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status = update
                .get("status")
                .and_then(|v| v.as_str())
                .map(normalize_tool_status);
            let title = update
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let content_text = extract_tool_content(update);
            vec![AgentEvent::ToolCallUpdate {
                id,
                status,
                title,
                content_text,
            }]
        }
        "plan" => {
            let entries = update
                .get("entries")
                .and_then(|e| e.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|e| PlanEntry {
                            content: e
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            priority: e
                                .get("priority")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            status: e
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            vec![AgentEvent::Plan { entries }]
        }
        // Official / Grok: turn finished (host must unlock even if RPC races)
        "turn_completed" | "prompt_complete" | "prompt_completed" | "turn_complete" => {
            let reason = update
                .get("stopReason")
                .or_else(|| update.get("stop_reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("end_turn")
                .to_string();
            vec![AgentEvent::TurnCompleted {
                stop_reason: reason,
            }]
        }
        // Token / context usage (several CLI spellings)
        "tokens_used" | "usage" | "context_usage" | "token_usage" | "context_compact"
        | "usage_update" => {
            let used = first_u64(
                update,
                &[
                    "tokensUsed",
                    "tokens_used",
                    "used",
                    "inputTokens",
                    "totalTokens",
                    "tokens_after",
                    "tokensAfter",
                ],
            );
            let max = first_u64(
                update,
                &[
                    "tokensMax",
                    "tokens_max",
                    "max",
                    "contextWindow",
                    "context_window",
                    "limit",
                ],
            );
            let before = first_u64(update, &["tokens_before", "tokensBefore"]);
            let after = first_u64(update, &["tokens_after", "tokensAfter"]);
            let used = used.or(after).or(before);
            let note = update
                .get("note")
                .or_else(|| update.get("summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if used.is_some() || max.is_some() || note.is_some() {
                vec![AgentEvent::Usage { used, max, note }]
            } else {
                Vec::new()
            }
        }
        _ => {
            // Best-effort: any update that carries token fields
            let used = first_u64(
                update,
                &["tokensUsed", "tokens_used", "tokens_after", "tokensAfter"],
            );
            let max = first_u64(update, &["contextWindow", "tokensMax", "tokens_max"]);
            if used.is_some() || max.is_some() {
                vec![AgentEvent::Usage {
                    used,
                    max,
                    note: None,
                }]
            } else {
                Vec::new()
            }
        }
    }
}

/// Collapse CLI status spellings into a small stable set for the UI.
pub fn normalize_tool_status(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "completed" | "complete" | "success" | "succeeded" | "done" | "ok" => "completed".into(),
        "failed" | "error" | "errored" => "failed".into(),
        "cancelled" | "canceled" => "cancelled".into(),
        "in_progress" | "running" | "active" => "in_progress".into(),
        "pending" | "queued" | "" => "pending".into(),
        other => other.to_string(),
    }
}

pub fn tool_status_is_terminal(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "complete" | "success" | "failed" | "error" | "cancelled" | "canceled"
    )
}

pub fn tool_status_is_running(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "pending" | "in_progress" | "running" | "queued" | "active" | ""
    )
}

fn first_u64(v: &Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(n) = v.get(*k).and_then(|x| {
            x.as_u64()
                .or_else(|| x.as_f64().map(|f| f as u64))
                .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        }) {
            return Some(n);
        }
    }
    None
}

fn extract_tool_content(update: &Value) -> Option<String> {
    if let Some(arr) = update.get("content").and_then(|c| c.as_array()) {
        let s = arr
            .iter()
            .filter_map(|item| {
                item.pointer("/content/text")
                    .and_then(|t| t.as_str())
                    .or_else(|| item.get("text").and_then(|t| t.as_str()))
                    .map(|s| s.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

pub fn extract_text_content(update: &Value) -> String {
    // Common ACP shapes (order matches RongleCat / Grok agent)
    if let Some(t) = update.pointer("/content/text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update.pointer("/content/delta").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update.get("content").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update.get("text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update.pointer("/delta/text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    if let Some(t) = update
        .pointer("/message/content/text")
        .and_then(|v| v.as_str())
    {
        return t.to_string();
    }
    // content: [{ type: "text", text: "..." }, ...]
    if let Some(arr) = update.get("content").and_then(|c| c.as_array()) {
        let s: String = arr
            .iter()
            .filter_map(|item| {
                let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                if ty != "text" && ty != "input_text" && ty != "output_text" {
                    return None;
                }
                item.get("text")
                    .and_then(|t| t.as_str())
                    .or_else(|| item.pointer("/content/text").and_then(|t| t.as_str()))
            })
            .collect();
        if !s.is_empty() {
            return s;
        }
    }
    String::new()
}

/// Build the JSON-RPC `session/prompt` params the desktop client sends.
pub fn build_prompt_params(session_id: &str, blocks: &[Value]) -> Value {
    serde_json::json!({
        "sessionId": session_id,
        "prompt": blocks
    })
}

/// Compact history bootstrap when session/load failed (RongleCat-style continuity).
pub fn build_history_bootstrap(
    turns: &[(bool, String)],
    max_turns: usize,
    max_chars: usize,
) -> String {
    let mut out =
        String::from("【以下为本地会话历史摘要，供上下文衔接；请据此继续，勿复述本段说明】\n\n");
    let slice: Vec<_> = turns.iter().rev().take(max_turns).collect();
    let slice: Vec<_> = slice.into_iter().rev().collect();
    let mut used = out.len();
    for (is_user, text) in slice {
        let role = if *is_user { "User" } else { "Assistant" };
        let mut body = text.chars().take(2000).collect::<String>();
        if text.chars().count() > 2000 {
            body.push_str("…");
        }
        let block = format!("{role}: {body}\n\n");
        if used + block.len() > max_chars {
            break;
        }
        out.push_str(&block);
        used += block.len();
    }
    out.push_str("【历史结束 · 请回应当前用户消息】\n");
    out
}
