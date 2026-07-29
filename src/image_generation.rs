//! Direct Grok Imagine image generation through the official xAI Images API.
//!
//! Grok Build CLI currently exposes coding/agent commands, not an image
//! generation subcommand. The desktop app therefore keeps this integration
//! separate from CLI OAuth and accepts an xAI API key for the current process
//! only. The key is never written to app config or logs.

use crate::attachments::PendingImage;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const API_KEY_URL: &str = "https://console.x.ai";
const GENERATIONS_URL: &str = "https://api.x.ai/v1/images/generations";
const MODEL: &str = "grok-imagine-image-quality";
const USER_AGENT: &str = "GrokDesktop-Imagine/0.1 (+https://github.com/qingchencloud/grok-app)";

#[derive(Debug, Clone)]
pub struct ImageGenerationRequest {
    pub api_key: String,
    pub prompt: String,
    pub aspect_ratio: String,
    pub resolution: String,
}

pub enum ImageGenerationEvent {
    Finished(Result<PendingImage, String>),
}

pub fn spawn_generate(request: ImageGenerationRequest, tx: Sender<ImageGenerationEvent>) {
    std::thread::Builder::new()
        .name("grok-image-generation".into())
        .spawn(move || {
            let result = generate(&request).map_err(|e| format!("{e:#}"));
            let _ = tx.send(ImageGenerationEvent::Finished(result));
        })
        .ok();
}

fn generate(request: &ImageGenerationRequest) -> Result<PendingImage> {
    let key = request.api_key.trim();
    let prompt = request.prompt.trim();
    if key.is_empty() {
        bail!("{}", crate::i18n::t().image_api_key_missing);
    }
    if prompt.is_empty() {
        bail!("{}", crate::i18n::t().image_prompt_hint);
    }

    let payload = serde_json::json!({
        "model": MODEL,
        "prompt": prompt,
        "n": 1,
        "aspect_ratio": normalize_aspect_ratio(&request.aspect_ratio),
        "resolution": normalize_resolution(&request.resolution),
        "response_format": "b64_json"
    });
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(180))
        .user_agent(USER_AGENT)
        .build();
    let response = match agent
        .post(GENERATIONS_URL)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .send_json(payload)
    {
        Ok(response) => response,
        Err(ureq::Error::Status(code, response)) => {
            let detail = response
                .into_json::<Value>()
                .ok()
                .and_then(|value| api_error_message(&value))
                .unwrap_or_else(|| format!("HTTP {code}"));
            return Err(anyhow!("{detail}"));
        }
        Err(error) => return Err(error).context("POST xAI Images API"),
    };

    let value: Value = response.into_json().context("parse xAI image response")?;
    let bytes = response_image_bytes(&value)?;
    crate::attachments::from_bytes(&bytes, "grok-imagine.png")
}

fn response_image_bytes(value: &Value) -> Result<Vec<u8>> {
    let first = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| anyhow!("{}: empty data", crate::i18n::t().image_generation_failed))?;
    let encoded = first
        .get("b64_json")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            let detail = api_error_message(value).unwrap_or_else(|| "missing b64_json".into());
            anyhow!("{}: {detail}", crate::i18n::t().image_generation_failed)
        })?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("decode generated image")
}

fn api_error_message(value: &Value) -> Option<String> {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_string)
}

fn normalize_aspect_ratio(value: &str) -> &'static str {
    match value.trim() {
        "1:1" => "1:1",
        "16:9" => "16:9",
        "9:16" => "9:16",
        "4:3" => "4:3",
        "3:4" => "3:4",
        "3:2" => "3:2",
        "2:3" => "2:3",
        _ => "auto",
    }
}

fn normalize_resolution(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "2k" => "2k",
        _ => "1k",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base64_image_payload() {
        let value = serde_json::json!({
            "data": [{ "b64_json": "aGVsbG8=" }]
        });
        assert_eq!(response_image_bytes(&value).unwrap(), b"hello");
    }

    #[test]
    fn normalizes_image_options() {
        assert_eq!(normalize_aspect_ratio("16:9"), "16:9");
        assert_eq!(normalize_aspect_ratio("unsupported"), "auto");
        assert_eq!(normalize_resolution("2K"), "2k");
        assert_eq!(normalize_resolution("8k"), "1k");
    }
}
