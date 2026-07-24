//! Thin harness: start ACP client, send one short prompt, log events (verification).
use grok_app::acp::{AcpClient, AgentEvent};
use grok_app::config::AppConfig;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let mut config = AppConfig::load();
    if config.cwd.trim().is_empty() {
        config.cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    println!("starting acp…");
    let client = match AcpClient::start(&config, tx).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FAIL connect: {e:#}");
            std::process::exit(2);
        }
    };
    println!("session={:?}", client.session_id());

    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Reply with exactly: pong".into());
    let cwd = config.cwd.clone();
    let client2 = client;
    let join = tokio::spawn(async move {
        match client2.prompt(&prompt, &cwd).await {
            Ok(stop) => println!("stop_reason={stop}"),
            Err(e) => eprintln!("prompt err: {e:#}"),
        }
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let mut got_message = false;
    let mut got_tool_or_thought = false;
    let mut chunks = String::new();
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Some(ev)) => match ev {
                AgentEvent::MessageChunk { text } => {
                    got_message = true;
                    chunks.push_str(&text);
                    print!("{text}");
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                AgentEvent::ThoughtChunk { text } => {
                    got_tool_or_thought = true;
                    println!("\n[thought] {}", text.chars().take(80).collect::<String>());
                }
                AgentEvent::ToolCall { title, kind, .. } => {
                    got_tool_or_thought = true;
                    println!("\n[tool] {title} ({kind})");
                }
                AgentEvent::PromptFinished { stop_reason } => {
                    println!("\n[finished] {stop_reason}");
                    break;
                }
                AgentEvent::Error { message } => {
                    eprintln!("\n[error] {message}");
                    break;
                }
                AgentEvent::AgentExited { code, pid } => {
                    eprintln!("\n[exit] code={code:?} pid={pid:?}");
                    break;
                }
                AgentEvent::Connected {
                    agent_name,
                    agent_version,
                } => {
                    println!("connected {agent_name} {agent_version}");
                }
                AgentEvent::SessionCreated { session_id } => {
                    println!("session created {session_id}");
                }
                AgentEvent::Log { message } => {
                    println!("[log] {message}");
                }
                _ => {}
            },
            Ok(None) => break,
            Err(_) => {
                if join.is_finished() {
                    // drain a bit more
                    while let Ok(ev) = rx.try_recv() {
                        if let AgentEvent::MessageChunk { text } = ev {
                            got_message = true;
                            chunks.push_str(&text);
                        }
                    }
                    break;
                }
            }
        }
    }
    let _ = join.await;
    println!("\n--- summary ---");
    println!("message_nonempty={}", !chunks.trim().is_empty());
    println!("got_message={got_message}");
    println!("got_tool_or_thought={got_tool_or_thought}");
    println!("chunk_len={}", chunks.len());
    if chunks.trim().is_empty() {
        std::process::exit(1);
    }
}
