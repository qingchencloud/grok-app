//! Read `~/.grok/models_cache.json` — same source as the official CLI catalog.
//! Used for context_window, reasoning efforts, and model labels.

use crate::config::grok_home;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// Context window tokens from CLI catalog (`info.context_window`).
    pub context_window: Option<u64>,
    pub supports_reasoning_effort: bool,
    pub reasoning_efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub auto_compact_threshold_percent: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelsCache {
    pub models: HashMap<String, ModelInfo>,
    pub default_model_id: Option<String>,
    pub origin: Option<String>,
    pub fetched_at: Option<String>,
    pub path: Option<PathBuf>,
}

static CACHE: Mutex<Option<(Instant, ModelsCache)>> = Mutex::new(None);
const TTL: Duration = Duration::from_secs(30);

fn cache_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(h) = grok_home() {
        out.push(h.join("models_cache.json"));
    }
    if let Ok(h) = std::env::var("GROK_HOME") {
        let p = PathBuf::from(h).join("models_cache.json");
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Load (or return cached) models catalog from disk.
pub fn load_models_cache() -> ModelsCache {
    if let Ok(guard) = CACHE.lock() {
        if let Some((at, ref c)) = *guard {
            if at.elapsed() < TTL {
                return c.clone();
            }
        }
    }

    let mut best = ModelsCache::default();
    for path in cache_paths() {
        if let Some(c) = parse_models_cache_file(&path) {
            best = c;
            break;
        }
    }

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), best.clone()));
    }
    best
}

/// Force re-read on next access (after model switch / CLI refresh).
pub fn invalidate_models_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

/// Merge a live ACP catalog (`_x.ai/models/update` / session/new models) into the
/// in-memory cache so context windows and effort lists stay fresh during a session.
pub fn merge_live_catalog(
    entries: &[crate::acp::ModelCatalogEntry],
    current_model_id: Option<&str>,
) {
    if entries.is_empty() {
        return;
    }
    let mut cache = load_models_cache();
    for e in entries {
        let info = ModelInfo {
            id: e.id.clone(),
            name: e.name.clone(),
            context_window: e.context_window.or_else(|| {
                cache
                    .models
                    .get(&e.id)
                    .and_then(|m| m.context_window)
            }),
            supports_reasoning_effort: e.supports_reasoning_effort
                || cache
                    .models
                    .get(&e.id)
                    .map(|m| m.supports_reasoning_effort)
                    .unwrap_or(false),
            reasoning_efforts: if e.reasoning_efforts.is_empty() {
                cache
                    .models
                    .get(&e.id)
                    .map(|m| m.reasoning_efforts.clone())
                    .unwrap_or_default()
            } else {
                e.reasoning_efforts.clone()
            },
            default_effort: e.default_effort.clone().or_else(|| {
                cache
                    .models
                    .get(&e.id)
                    .and_then(|m| m.default_effort.clone())
            }),
            auto_compact_threshold_percent: cache
                .models
                .get(&e.id)
                .and_then(|m| m.auto_compact_threshold_percent),
        };
        cache.models.insert(e.id.clone(), info);
    }
    if let Some(id) = current_model_id {
        if !id.is_empty() {
            cache.default_model_id = Some(id.to_string());
        }
    }
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((Instant::now(), cache));
    }
}

fn parse_models_cache_file(path: &PathBuf) -> Option<ModelsCache> {
    let raw = std::fs::read_to_string(path).ok()?;
    parse_models_cache_json(&raw, Some(path.clone()))
}

pub fn parse_models_cache_json(raw: &str, path: Option<PathBuf>) -> Option<ModelsCache> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let models_obj = v.get("models")?.as_object()?;
    let mut models = HashMap::new();
    let mut first_id = None;

    for (id, body) in models_obj {
        if id.trim().is_empty() {
            continue;
        }
        let info = body.get("info").unwrap_or(body);
        let hidden = info
            .get("hidden")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if hidden {
            continue;
        }
        let name = info
            .get("name")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(id)
            .to_string();
        let context_window = info
            .get("context_window")
            .or_else(|| info.get("contextWindow"))
            .and_then(|x| {
                x.as_u64()
                    .or_else(|| x.as_f64().map(|f| f as u64))
                    .or_else(|| x.as_i64().map(|i| i.max(0) as u64))
            });
        let supports_reasoning_effort = info
            .get("supports_reasoning_effort")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let mut reasoning_efforts = Vec::new();
        let mut default_effort = None;
        if let Some(arr) = info.get("reasoning_efforts").and_then(|a| a.as_array()) {
            for e in arr {
                let eid = e
                    .get("id")
                    .or_else(|| e.get("value"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if eid.is_empty() {
                    continue;
                }
                if e.get("default").and_then(|x| x.as_bool()).unwrap_or(false) {
                    default_effort = Some(eid.clone());
                }
                reasoning_efforts.push(eid);
            }
        }
        if default_effort.is_none() {
            if let Some(re) = info.get("reasoning_effort").and_then(|x| x.as_str()) {
                default_effort = Some(re.to_string());
            }
        }
        let auto_compact_threshold_percent = info
            .get("auto_compact_threshold_percent")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32);

        if first_id.is_none() {
            first_id = Some(id.clone());
        }
        models.insert(
            id.clone(),
            ModelInfo {
                id: id.clone(),
                name,
                context_window,
                supports_reasoning_effort,
                reasoning_efforts,
                default_effort,
                auto_compact_threshold_percent,
            },
        );
    }

    if models.is_empty() {
        return None;
    }

    let default_model_id = models
        .keys()
        .find(|k| k.as_str() == "grok-4.5")
        .cloned()
        .or(first_id);

    Some(ModelsCache {
        models,
        default_model_id,
        origin: v
            .get("origin")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        fetched_at: v
            .get("fetched_at")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        path,
    })
}

/// Context window for a model id from CLI cache. `None` if unknown.
pub fn context_window_for(model_id: &str) -> Option<u64> {
    let cache = load_models_cache();
    let id = model_id.trim();
    if id.is_empty() {
        return cache
            .default_model_id
            .as_ref()
            .and_then(|d| cache.models.get(d))
            .and_then(|m| m.context_window);
    }
    cache
        .models
        .get(id)
        .or_else(|| {
            // case-insensitive match
            cache
                .models
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(id))
                .map(|(_, v)| v)
        })
        .and_then(|m| m.context_window)
}

/// List model ids from cache (for picker); falls back to empty.
pub fn catalog_model_ids() -> Vec<String> {
    let cache = load_models_cache();
    let mut ids: Vec<String> = cache.models.keys().cloned().collect();
    ids.sort();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_context_window() {
        let raw = r#"{
          "models": {
            "grok-4.5": {
              "info": {
                "id": "grok-4.5",
                "name": "Grok 4.5",
                "context_window": 500000,
                "hidden": false,
                "supports_reasoning_effort": true,
                "reasoning_efforts": [
                  { "id": "high", "default": true },
                  { "id": "medium" },
                  { "id": "low" }
                ]
              }
            }
          }
        }"#;
        let c = parse_models_cache_json(raw, None).expect("parse");
        assert_eq!(
            c.models.get("grok-4.5").and_then(|m| m.context_window),
            Some(500_000)
        );
    }
}
