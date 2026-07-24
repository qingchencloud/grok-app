//! Unit tests drive shipped pure modules (sessions, attachments, config, ACP parse).

use std::path::PathBuf;

use grok_app::acp::parse::{build_prompt_params, session_update_to_events};
use grok_app::acp::{AgentEvent, InboundMessage};
use grok_app::attachments::{build_prompt_blocks, from_bytes, from_paste_payload};

#[test]
fn inbound_request_not_misparsed_as_response() {
    // Regression: untagged enum used to match Response first (optional result/error),
    // swallowing fs/read_text_file → tools hung with xaiAcpChannelFailure recv_failed.
    let line = r#"{"jsonrpc":"2.0","id":42,"method":"fs/read_text_file","params":{"path":"Cargo.toml"}}"#;
    let msg: InboundMessage = serde_json::from_str(line).expect("parse request");
    match msg {
        InboundMessage::Request { method, id, .. } => {
            assert_eq!(method, "fs/read_text_file");
            assert_eq!(id, serde_json::json!(42));
        }
        other => panic!("expected Request, got {other:?}"),
    }

    let resp_line = r#"{"jsonrpc":"2.0","id":7,"result":{"stopReason":"end_turn"}}"#;
    let msg: InboundMessage = serde_json::from_str(resp_line).expect("parse response");
    match msg {
        InboundMessage::Response(r) => {
            assert_eq!(r.id, serde_json::json!(7));
            assert!(r.result.is_some());
        }
        other => panic!("expected Response, got {other:?}"),
    }

    let note = r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi"}}}}"#;
    let msg: InboundMessage = serde_json::from_str(note).expect("parse notification");
    match msg {
        InboundMessage::Notification(n) => assert_eq!(n.method, "session/update"),
        other => panic!("expected Notification, got {other:?}"),
    }
}
use grok_app::local::cli_config::{merge_config_toml, CliTomlConfig};
use grok_app::local::sessions::{
    group_sessions_by_project, normalize_project_key, parse_summary_json, parse_updates_jsonl,
    LocalSession,
};

/// Valid 1×1 red PNG (CRC-correct, generated via zlib).
fn tiny_png() -> &'static [u8] {
    &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92, 0xEF, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ]
}

#[test]
fn parse_real_summary_shape() {
    let json = r#"{
      "info": {
        "id": "019f8326-1363-7873-a7e2-bba75fea1983",
        "cwd": "C:\\Users\\keh5"
      },
      "session_summary": "Chinese Greeting Hello Session Start",
      "created_at": "2026-07-21T05:28:58.471526800Z",
      "updated_at": "2026-07-21T05:31:50.395703200Z",
      "num_messages": 44,
      "num_chat_messages": 29,
      "current_model_id": "grok-4.5",
      "generated_title": "Chinese Greeting Hello Session Start"
    }"#;
    let dir = PathBuf::from("dummy_session_dir");
    let summary = PathBuf::from("dummy_session_dir/summary.json");
    let s = parse_summary_json(json, &summary, &dir).expect("parse summary");
    assert_eq!(s.id, "019f8326-1363-7873-a7e2-bba75fea1983");
    assert_eq!(s.title, "Chinese Greeting Hello Session Start");
    assert_eq!(s.cwd, "C:\\Users\\keh5");
    assert_eq!(s.model, "grok-4.5");
    assert_eq!(s.num_messages, 29);
    assert!(s.updated_at.is_some());
}

#[test]
fn group_sessions_by_project_dir() {
    use chrono::TimeZone;
    let t1 = chrono::Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap();
    let t2 = chrono::Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap();
    let sessions = vec![
        LocalSession {
            id: "a".into(),
            title: "A1".into(),
            cwd: r"D:\Data\Rust\GrokApp".into(),
            model: "grok-4.5".into(),
            created_at: Some(t1),
            updated_at: Some(t1),
            num_messages: 1,
            path: Default::default(),
            summary_path: Default::default(),
        },
        LocalSession {
            id: "b".into(),
            title: "B1".into(),
            cwd: r"D:/Data/Go/TKCPO-Next".into(),
            model: "grok-4.5".into(),
            created_at: Some(t2),
            updated_at: Some(t2),
            num_messages: 2,
            path: Default::default(),
            summary_path: Default::default(),
        },
        LocalSession {
            id: "c".into(),
            title: "A2".into(),
            cwd: r"D:\Data\Rust\GrokApp\".into(),
            model: "grok-4.5".into(),
            created_at: Some(t2),
            updated_at: Some(t2),
            num_messages: 3,
            path: Default::default(),
            summary_path: Default::default(),
        },
    ];
    let groups = group_sessions_by_project(&sessions, Some(r"D:\Data\Rust\GrokApp"));
    assert_eq!(groups.len(), 2);
    // preferred project first
    assert_eq!(groups[0].name, "GrokApp");
    assert_eq!(groups[0].sessions.len(), 2);
    assert_eq!(groups[1].name, "TKCPO-Next");
    assert_eq!(
        normalize_project_key(r"D:/Data/Rust/GrokApp/"),
        normalize_project_key(r"D:\Data\Rust\GrokApp")
    );
}

#[test]
fn parse_updates_keeps_last_n_not_first() {
    // 5 user messages; max_items=2 should keep the last two user flushes
    let mut lines = String::new();
    for i in 0..5 {
        lines.push_str(&format!(
            r#"{{"params":{{"update":{{"sessionUpdate":"user_message_chunk","content":{{"type":"text","text":"msg{i}"}}}}}}}}"#
        ));
        lines.push('\n');
        lines.push_str(&format!(
            r#"{{"params":{{"update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"ans{i}"}}}}}}}}"#
        ));
        lines.push('\n');
    }
    let items = parse_updates_jsonl(&lines, 2);
    assert_eq!(items.len(), 2, "expected last 2 items, got {items:?}");
    // last two should be around msg4/ans4
    let texts: Vec<String> = items
        .iter()
        .map(|i| match i {
            grok_app::acp::TimelineItem::UserMessage { text, .. } => text.clone(),
            grok_app::acp::TimelineItem::AssistantMessage { text, .. } => text.clone(),
            _ => String::new(),
        })
        .collect();
    assert!(
        texts.iter().any(|t| t.contains('4')),
        "should keep recent msgs, got {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.contains("msg0")),
        "should not keep first msg: {texts:?}"
    );
}

#[test]
fn parse_updates_jsonl_streams_user_and_assistant() {
    let lines = r#"
{"params":{"update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"你好"}}}}
{"params":{"update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"thinking…"}}}}
{"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Hello "}}}}
{"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"world"}}}}
{"params":{"update":{"sessionUpdate":"tool_call","toolCallId":"t1","title":"read_file","kind":"read"}}}
"#;
    let items = parse_updates_jsonl(lines, 50);
    assert!(
        items.iter().any(|i| matches!(i, grok_app::acp::TimelineItem::UserMessage { text, .. } if text.contains("你好"))),
        "user message missing: {items:?}"
    );
    assert!(
        items.iter().any(|i| matches!(i, grok_app::acp::TimelineItem::AssistantMessage { text, .. } if text.contains("Hello") && text.contains("world"))),
        "assistant merge missing: {items:?}"
    );
    assert!(
        items.iter().any(|i| matches!(i, grok_app::acp::TimelineItem::Tool { title, .. } if title == "read_file")),
        "tool missing: {items:?}"
    );
    assert!(
        items.iter().any(|i| matches!(i, grok_app::acp::TimelineItem::Thought { text, .. } if text.contains("thinking"))),
        "thought missing: {items:?}"
    );
}

#[test]
fn paste_payload_data_uri() {
    // 1x1 red png as data URI
    let png = tiny_png();
    let b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        png,
    );
    let uri = format!("data:image/png;base64,{b64}");
    let img = from_paste_payload(&uri).expect("data URI paste");
    assert!(img.png_bytes.len() > 8);
    assert_eq!(img.width, 1);
    assert_eq!(img.height, 1);
}

#[test]
fn image_bytes_to_acp_blocks_with_text() {
    let png = tiny_png();
    let img = from_bytes(png, "pixel.png").expect("decode 1x1 png via shipped path");
    assert!(img.png_bytes.len() > 8, "png re-encode empty");
    assert_eq!(&img.png_bytes[0..8], b"\x89PNG\r\n\x1a\n");
    let blocks = build_prompt_blocks("describe this", &[img]);
    assert!(
        blocks
            .iter()
            .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("image")),
        "missing image block: {blocks:?}"
    );
    let image = blocks
        .iter()
        .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("image"))
        .unwrap();
    let data = image.get("data").and_then(|d| d.as_str()).unwrap_or("");
    assert!(!data.is_empty(), "base64 empty");
    assert_eq!(
        image.get("mimeType").and_then(|m| m.as_str()),
        Some("image/png")
    );
    assert!(
        blocks.iter().any(|b| {
            b.get("type").and_then(|t| t.as_str()) == Some("text")
                && b.get("text").and_then(|t| t.as_str()) == Some("describe this")
        }),
        "missing text block: {blocks:?}"
    );
}

#[test]
fn image_only_prompt_gets_default_text_hint() {
    let img = from_bytes(tiny_png(), "only.png").unwrap();
    let blocks = build_prompt_blocks("   ", &[img]);
    assert!(blocks
        .iter()
        .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("image")));
    assert!(blocks
        .iter()
        .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")));
}

#[test]
fn config_toml_round_trip_preserves_unrelated_keys() {
    let existing = r#"
[cli]
auto_update = false
installer = "internal"

[custom]
keep_me = "yes"

[ui]
permission_mode = "default"
yolo = false
"#;
    let mut cfg = CliTomlConfig::from_toml_str(existing);
    assert!(!cfg.auto_update);
    assert_eq!(cfg.permission_mode, "default");
    cfg.permission_mode = "always-approve".into();
    cfg.yolo = true;
    cfg.default_model = "grok-4.5".into();
    cfg.auto_compact_threshold_percent = 80;

    let merged = merge_config_toml(existing, &cfg);
    assert!(
        merged.contains("keep_me"),
        "unrelated key clobbered:\n{merged}"
    );
    assert!(
        merged.contains("installer"),
        "cli.installer clobbered:\n{merged}"
    );
    let again = CliTomlConfig::from_toml_str(&merged);
    assert_eq!(again.permission_mode, "always-approve");
    assert!(again.yolo);
    assert_eq!(again.default_model, "grok-4.5");
    assert_eq!(again.auto_compact_threshold_percent, 80);
}

#[test]
fn session_update_parser_emits_message_thought_tool() {
    let msg = serde_json::json!({
        "sessionUpdate": "agent_message_chunk",
        "content": { "type": "text", "text": "hi" }
    });
    let thought = serde_json::json!({
        "sessionUpdate": "agent_thought_chunk",
        "content": { "type": "text", "text": "reason" }
    });
    let tool = serde_json::json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "c1",
        "title": "grep",
        "kind": "search",
        "status": "pending"
    });
    let e1 = session_update_to_events(&msg);
    assert!(matches!(e1.as_slice(), [AgentEvent::MessageChunk { text }] if text == "hi"));
    let e2 = session_update_to_events(&thought);
    assert!(matches!(e2.as_slice(), [AgentEvent::ThoughtChunk { text }] if text == "reason"));
    let e3 = session_update_to_events(&tool);
    assert!(matches!(
        e3.as_slice(),
        [AgentEvent::ToolCall { title, kind, .. }] if title == "grep" && kind == "search"
    ));
}

#[test]
fn build_prompt_params_shape() {
    let blocks = vec![
        serde_json::json!({"type":"text","text":"hello"}),
        serde_json::json!({"type":"image","mimeType":"image/png","data":"abc"}),
    ];
    let p = build_prompt_params("sess-1", &blocks);
    assert_eq!(p.get("sessionId").and_then(|s| s.as_str()), Some("sess-1"));
    let prompt = p.get("prompt").and_then(|p| p.as_array()).unwrap();
    assert_eq!(prompt.len(), 2);
    assert_eq!(prompt[0]["type"], "text");
    assert_eq!(prompt[1]["type"], "image");
}
