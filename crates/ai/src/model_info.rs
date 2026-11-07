//! What a model can do, as far as its provider's model list says, and a short-lived memory of
//! those lists so a turn can shape its request without asking again.
//!
//! The memory is filled whenever a provider's list is read with its details (the model picker,
//! a backend test, a key check, all off the UI thread) and, for a client that does so, once
//! before its first turn on a worker thread with a short deadline. A list that couldn't be read
//! is remembered as empty for a few minutes, so a turn never waits on it again; empty means
//! "unknown", which keeps the request as it was.

use crate::model::Effort;
use craft_anything_platform::sync::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

/// What a model can do, as far as its provider says (`None` = not said, never "no").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    /// Input context window, in tokens.
    pub context_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub tools: Option<bool>,
    pub vision: Option<bool>,
    /// Whether it thinks before answering (for Claude: adaptive thinking is accepted).
    pub reasoning: Option<bool>,
    /// The effort levels it accepts; `Some(vec![])` means it takes no effort setting.
    pub efforts: Option<Vec<Effort>>,
    /// A router's named chain of models rather than one model.
    pub is_combo: bool,
    pub deprecated: bool,
}

/// Longest model id or display name kept from a provider.
pub const MAX_NAME_CHARS: usize = 256;
/// Token counts above this are not believed (no model is near it).
const MAX_PLAUSIBLE_TOKENS: u64 = 1 << 32;

/// A token count from a provider: a positive whole number of plausible size, else unknown.
pub fn token_count(value: Option<&serde_json::Value>) -> Option<u64> {
    value.and_then(serde_json::Value::as_u64).filter(|n| (1..=MAX_PLAUSIBLE_TOKENS).contains(n))
}

/// A provider's name or id, trimmed and bounded; empty is unknown.
pub fn bounded_name(value: Option<&serde_json::Value>) -> Option<String> {
    let text = value.and_then(serde_json::Value::as_str)?.trim();
    (!text.is_empty()).then(|| text.chars().take(MAX_NAME_CHARS).collect())
}

/// How long a list read with its details is trusted.
pub const LIST_TTL: Duration = Duration::from_secs(60 * 60);
/// How long a list that couldn't be read stays unknown before another try.
pub const FAILED_TTL: Duration = Duration::from_secs(5 * 60);
/// Most services remembered at once.
const MAX_SERVICES: usize = 32;

struct Remembered {
    base_url: String,
    expires: Instant,
    models: Vec<ModelInfo>,
}

/// Recently read model lists, by service.
#[derive(Default)]
struct Memory(Vec<Remembered>);

impl Memory {
    fn remember(&mut self, base_url: &str, models: Vec<ModelInfo>, ttl: Duration, now: Instant) {
        let base_url = key(base_url);
        let Some(expires) = now.checked_add(ttl) else { return };
        self.0.retain(|r| r.expires > now && r.base_url != base_url);
        if self.0.len() >= MAX_SERVICES
            && let Some(oldest) = self.0.iter().enumerate().min_by_key(|(_, r)| r.expires).map(|(i, _)| i)
        {
            self.0.swap_remove(oldest);
        }
        self.0.push(Remembered { base_url, expires, models });
    }

    fn recall(&self, base_url: &str, model: &str, now: Instant) -> Known {
        let base_url = key(base_url);
        let model = model.trim();
        match self.0.iter().find(|r| r.base_url == base_url && r.expires > now) {
            Some(r) => Known::Listed(r.models.iter().find(|m| m.id == model).cloned()),
            None => Known::NotAsked,
        }
    }
}

static MEMORY: Mutex<Memory> = Mutex::new(Memory(Vec::new()));

/// What the memory says about a model at a service.
#[derive(Clone, Debug, PartialEq)]
pub enum Known {
    /// The service's list was read recently; the model's entry, if it had one.
    Listed(Option<ModelInfo>),
    /// Nothing recent about this service.
    NotAsked,
}

fn key(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').trim_end_matches("/v1").to_ascii_lowercase()
}

/// Remember the models a service listed (empty for a list that couldn't be read) for `ttl`.
pub fn remember(base_url: &str, models: Vec<ModelInfo>, ttl: Duration) {
    MEMORY.lock().unwrap_or_else(PoisonError::into_inner).remember(base_url, models, ttl, Instant::now());
}

/// What is remembered about `model` at `base_url`.
pub fn recall(base_url: &str, model: &str) -> Known {
    MEMORY.lock().unwrap_or_else(PoisonError::into_inner).recall(base_url, model, Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hostile_numbers_and_names_are_unknown() {
        for bad in [json!("x"), json!(-5), json!(0), json!(1.5), json!(1e30), json!(u64::MAX), json!(null), json!({"n": 1})] {
            assert_eq!(token_count(Some(&bad)), None, "{bad}");
        }
        assert_eq!(token_count(Some(&json!(1_000_000))), Some(1_000_000));
        assert_eq!(token_count(None), None);
        assert_eq!(bounded_name(Some(&json!("  "))), None);
        assert_eq!(bounded_name(Some(&json!(7))), None);
        assert_eq!(bounded_name(Some(&json!("é".repeat(1000)))).map(|s| s.chars().count()), Some(MAX_NAME_CHARS));
    }

    #[test]
    fn the_memory_is_per_service_bounded_and_expires() {
        let model = |id: &str| ModelInfo { id: id.into(), ..Default::default() };
        let now = Instant::now();
        let mut memory = Memory::default();
        let base = "https://memory-one.test/v1";
        assert_eq!(memory.recall(base, "a", now), Known::NotAsked);
        memory.remember(base, vec![model("a")], LIST_TTL, now);
        assert_eq!(memory.recall("https://MEMORY-one.test/", "a", now), Known::Listed(Some(model("a"))));
        assert_eq!(memory.recall(base, "b", now), Known::Listed(None), "listed, but not this model");
        assert_eq!(memory.recall(base, "a", now + LIST_TTL), Known::NotAsked, "an hour later it is asked again");
        memory.remember(base, Vec::new(), FAILED_TTL, now);
        assert_eq!(memory.recall(base, "a", now), Known::Listed(None), "a newer entry replaces the older one");
        for i in 0..(MAX_SERVICES * 2) {
            memory.remember(&format!("https://memory-many-{i}.test"), vec![model("x")], LIST_TTL + Duration::from_secs(i as u64), now);
        }
        assert_eq!(memory.0.len(), MAX_SERVICES);
        assert_eq!(memory.recall("https://memory-many-63.test", "x", now), Known::Listed(Some(model("x"))), "the newest stays");
        assert_eq!(memory.recall("https://memory-many-0.test", "x", now), Known::NotAsked, "the soonest to expire went");
        memory.remember("https://memory-far.test", Vec::new(), Duration::MAX, now);
        assert_eq!(memory.recall("https://memory-far.test", "x", now), Known::NotAsked, "an unrepresentable expiry isn't kept");
        remember("https://memory-global.test", vec![model("g")], LIST_TTL);
        assert_eq!(recall("https://memory-global.test/v1", "g"), Known::Listed(Some(model("g"))));
    }
}
