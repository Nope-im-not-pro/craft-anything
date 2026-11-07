//! Anthropic Messages API: streaming SSE, tools, adaptive thinking, effort, prompt caching and
//! server-side fallbacks.

use crate::http::{self, SseEvent};
use crate::model::{Block, ChatModel, Delta, Effort, Msg, Speaker, Stop, StopReason, ToolDef, TurnRequest, TurnResult, Usage};
use crate::model_info::{self, Known, ModelInfo};
use craft_anything_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub const DEFAULT_BASE: &str = "https://api.anthropic.com";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

/// Whether an address is Anthropic's own API, which speaks this protocol and not OpenAI's (#1100).
pub fn is_anthropic_api(address: &str) -> bool {
    crate::openai::https_host_path(address).is_some_and(|(host, _)| host.eq_ignore_ascii_case("api.anthropic.com"))
}
/// Offered in the model dropdown when the live list can't be fetched.
pub const MODELS: &[&str] = &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"];
/// Models that offer fewer effort levels than all five (Claude's effort guide), by model id
/// prefix; any other model is sent the level as chosen.
pub const EFFORT_LEVELS: &[(&str, &[Effort])] = &[
    ("claude-opus-4-6", &[Effort::Low, Effort::Medium, Effort::High, Effort::Max]),
    ("claude-sonnet-4-6", &[Effort::Low, Effort::Medium, Effort::High, Effort::Max]),
    ("claude-opus-4-5", &[Effort::Low, Effort::Medium, Effort::High]),
];

/// The effort level `model` is sent: `effort`, or the nearest level below it the model has.
pub fn effort_for(model: &str, effort: Effort) -> Effort {
    let effort = effort.within(&Effort::ALL);
    let model = model.trim().trim_start_matches("anthropic/").trim_start_matches("anthropic.");
    EFFORT_LEVELS.iter().find(|(prefix, _)| model.starts_with(prefix)).map_or(effort, |(_, levels)| effort.within(levels))
}

/// The effort level sent, judged by what the model's list entry says when we have one: none for
/// a model that takes no effort setting, the nearest level it has otherwise, and
/// [`effort_for`]'s table when its levels are unknown.
pub fn effort_with(model: &str, effort: Effort, info: Option<&ModelInfo>) -> Option<Effort> {
    match info.and_then(|i| i.efforts.as_deref()) {
        Some([]) => None,
        Some(levels) => Some(effort.within(&Effort::ALL).within(levels)),
        None => Some(effort_for(model, effort)),
    }
}
pub const API_VERSION: &str = "2023-06-01";
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
pub const MAX_TOKENS: u32 = 64_000;

/// Most content blocks we track in one response.
const MAX_BLOCKS: usize = 4096;
/// Most pages of `/v1/models` read, and most models kept from them.
const MAX_MODEL_PAGES: usize = 5;
const MAX_MODELS: usize = 5000;
/// How long a turn waits to read a model list it hasn't seen this hour.
const TURN_LIST_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Anthropic {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Send the server-side fallback header and body field (Anthropic's own API only).
    pub fallbacks: bool,
    /// How long one streamed answer may take.
    pub stream_limit: std::time::Duration,
    /// Before a turn, read the model list (at most once an hour, with a short deadline) to learn
    /// which effort levels and thinking the model takes. On for Anthropic's own API.
    pub learn_capabilities: bool,
}

impl Anthropic {
    pub fn new(base_url: Option<&str>, api_key: String, model: &str) -> Self {
        let base_url = base_url.unwrap_or(DEFAULT_BASE).trim_end_matches('/').to_string();
        Anthropic {
            learn_capabilities: is_anthropic_api(&base_url),
            base_url,
            api_key,
            model: if model.is_empty() { DEFAULT_MODEL.to_string() } else { model.to_string() },
            fallbacks: true,
            stream_limit: http::STREAM_BODY,
        }
    }

    fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches("/v1");
        format!("{base}/v1/{path}")
    }

    /// Server-side fallbacks are a beta of Anthropic's own API; Bedrock, Vertex and proxies answer
    /// 400 to its header.
    fn sends_fallbacks(&self) -> bool {
        self.fallbacks && is_anthropic_api(&self.base_url)
    }

    pub fn headers(&self) -> Vec<(&'static str, String)> {
        let mut h = vec![("x-api-key", self.api_key.clone()), ("anthropic-version", API_VERSION.to_string())];
        if self.sends_fallbacks() {
            h.push(("anthropic-beta", FALLBACK_BETA.to_string()));
        }
        h
    }

    /// The request body for one streamed turn.
    pub fn body(&self, req: &TurnRequest<'_>) -> Value {
        let info = match model_info::recall(&self.base_url, &self.model) {
            Known::Listed(info) => info,
            Known::NotAsked => None,
        };
        let mut body = json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "stream": true,
            "system": [{"type": "text", "text": req.system, "cache_control": {"type": "ephemeral"}}],
            "messages": req.messages.iter().map(message_json).collect::<Vec<_>>(),
        });
        if let Some(m) = body.as_object_mut() {
            // Unknown is sent as before; only a model that says it can't is spared the setting.
            if info.as_ref().and_then(|i| i.reasoning) != Some(false) {
                m.insert("thinking".into(), json!({"type": "adaptive", "display": "summarized"}));
            }
            if let Some(effort) = effort_with(&self.model, req.effort, info.as_ref()) {
                m.insert("output_config".into(), json!({"effort": effort.as_str()}));
            }
        }
        if !req.tools.is_empty() {
            let n = req.tools.len();
            let tools: Vec<Value> = req
                .tools
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut v = json!({"name": t.name, "description": t.description, "input_schema": t.schema, "eager_input_streaming": true});
                    if i + 1 == n
                        && let Some(m) = v.as_object_mut()
                    {
                        m.insert("cache_control".into(), json!({"type": "ephemeral"}));
                    }
                    v
                })
                .collect();
            if let Some(m) = body.as_object_mut() {
                m.insert("tools".into(), Value::Array(tools));
                m.insert("tool_choice".into(), json!({"type": "auto"}));
            }
        }
        if self.sends_fallbacks()
            && let Some(m) = body.as_object_mut()
        {
            m.insert("fallbacks".into(), json!("default"));
        }
        body
    }

    /// Model ids available to this key.
    pub fn list_models(&self, timeout: Duration) -> Result<Vec<String>, ToolError> {
        Ok(self.list_model_info(timeout)?.into_iter().map(|m| m.id).collect())
    }

    /// The models available to this key with what each can do, read page by page (at most
    /// [`MAX_MODEL_PAGES`] pages and [`MAX_MODELS`] models, all within `timeout`). The list is
    /// remembered for an hour so turns can shape their requests by it.
    pub fn list_model_info(&self, timeout: Duration) -> Result<Vec<ModelInfo>, ToolError> {
        let deadline = Instant::now().checked_add(timeout);
        let headers = self.headers();
        let mut models: Vec<ModelInfo> = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        let mut after: Option<String> = None;
        for page_number in 0..MAX_MODEL_PAGES {
            let left = deadline.map_or(timeout, |d| d.saturating_duration_since(Instant::now()));
            if left.is_zero() {
                if page_number == 0 {
                    return Err(ToolError::new(ErrorCode::TimedOut, "Anthropic's model list took too long.").with_remediation("Try again."));
                }
                break;
            }
            let mut url = format!("{}?limit=1000", self.url("models"));
            if let Some(id) = &after {
                url.push_str("&after_id=");
                url.push_str(&query_value(id));
            }
            let page = match http::get_json(&http::quick_agent(left), "Anthropic", &url, &headers) {
                Ok(page) => page,
                Err(e) if page_number == 0 => return Err(e),
                // A later page that fails leaves the models already read.
                Err(e) => {
                    log::warn!("Anthropic's model list stopped after {page_number} pages: {}", e.message);
                    break;
                }
            };
            for entry in page.get("data").and_then(Value::as_array).into_iter().flatten() {
                if models.len() >= MAX_MODELS {
                    break;
                }
                if let Some(info) = model_entry(entry)
                    && ids.insert(info.id.clone())
                {
                    models.push(info);
                }
            }
            let next = model_info::bounded_name(page.get("last_id"));
            match (page.get("has_more").and_then(Value::as_bool), next) {
                (Some(true), Some(next)) if models.len() < MAX_MODELS && after.as_ref() != Some(&next) => after = Some(next),
                _ => break,
            }
        }
        model_info::remember(&self.base_url, models.clone(), model_info::LIST_TTL);
        Ok(models)
    }

    /// Learn the model's capabilities before a turn when this hour's list hasn't been read. It
    /// runs on the conversation's worker thread with a short deadline; a list that can't be read
    /// is remembered as unknown for a few minutes, which sends the request as before.
    fn learn(&self) {
        if !self.learn_capabilities || !matches!(model_info::recall(&self.base_url, &self.model), Known::NotAsked) {
            return;
        }
        if let Err(e) = self.list_model_info(TURN_LIST_WAIT) {
            log::info!("couldn't read Anthropic's model list for its capabilities: {}", e.message);
            model_info::remember(&self.base_url, Vec::new(), model_info::FAILED_TTL);
        }
    }
}

/// A query-string value with everything but unreserved characters percent-encoded.
fn query_value(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// One `/v1/models` entry. Anything missing, of the wrong type or implausible is unknown.
fn model_entry(entry: &Value) -> Option<ModelInfo> {
    let id = model_info::bounded_name(entry.get("id"))?;
    let caps = entry.get("capabilities").filter(|c| c.is_object());
    let supported = |path: &str| caps.and_then(|c| c.pointer(path)).and_then(|v| v.get("supported")).and_then(Value::as_bool);
    let thinking = caps.and_then(|c| c.get("thinking")).and_then(|t| t.get("supported")).and_then(Value::as_bool);
    let adaptive = match (supported("/thinking/types/adaptive"), thinking) {
        (Some(said), _) => Some(said),
        (None, Some(false)) => Some(false),
        (None, _) => None,
    };
    Some(ModelInfo {
        display_name: model_info::bounded_name(entry.get("display_name")),
        context_tokens: model_info::token_count(entry.get("max_input_tokens")),
        max_output_tokens: model_info::token_count(entry.get("max_tokens")),
        tools: None,
        vision: supported("/image_input"),
        reasoning: adaptive,
        efforts: caps.and_then(|c| c.get("effort")).and_then(effort_levels),
        is_combo: false,
        deprecated: matches!(entry.get("lifecycle").and_then(Value::as_str), Some("deprecated" | "retired")),
        id,
    })
}

/// `capabilities.effort`: no levels when it isn't supported, the levels marked supported when any
/// level is described, else unknown.
fn effort_levels(effort: &Value) -> Option<Vec<Effort>> {
    if effort.get("supported").and_then(Value::as_bool) == Some(false) {
        return Some(Vec::new());
    }
    let said: Vec<(Effort, bool)> = Effort::ALL
        .into_iter()
        .filter_map(|level| effort.get(level.as_str()).and_then(|v| v.get("supported")).and_then(Value::as_bool).map(|s| (level, s)))
        .collect();
    (!said.is_empty()).then(|| said.into_iter().filter(|(_, s)| *s).map(|(level, _)| level).collect())
}

pub fn message_json(m: &Msg) -> Value {
    let role = match m.role {
        Speaker::User => "user",
        Speaker::Assistant => "assistant",
    };
    let content: Vec<Value> = m
        .blocks
        .iter()
        .filter_map(|b| {
            Some(match b {
                Block::Text { text } => json!({"type": "text", "text": text}),
                Block::Thinking { thinking, signature } => json!({"type": "thinking", "thinking": thinking, "signature": signature}),
                Block::RedactedThinking { data } => json!({"type": "redacted_thinking", "data": data}),
                Block::ToolUse { id, name, input } => json!({"type": "tool_use", "id": id, "name": name, "input": input}),
                Block::ToolExtra { .. } => return None,
                Block::ToolResult { tool_use_id, content, is_error } => {
                    json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content, "is_error": is_error})
                }
            })
        })
        .collect();
    json!({"role": role, "content": content})
}

#[derive(Debug, Default)]
enum Partial {
    #[default]
    Empty,
    Text(String),
    Thinking(String, String),
    Redacted(String),
    Tool {
        id: String,
        name: String,
        json: String,
    },
}

/// Accumulates one streamed response.
#[derive(Debug, Default)]
pub struct StreamState {
    blocks: Vec<Partial>,
    pub usage: Usage,
    pub stop: Option<StopReason>,
    pub model: String,
    pub done: bool,
}

impl StreamState {
    fn slot(&mut self, index: usize) -> Option<&mut Partial> {
        if index >= MAX_BLOCKS {
            return None;
        }
        if self.blocks.len() <= index {
            self.blocks.resize_with(index + 1, Partial::default);
        }
        self.blocks.get_mut(index)
    }

    /// Apply one SSE event.
    pub fn apply(&mut self, ev: &SseEvent, on_delta: &mut dyn FnMut(Delta)) -> Result<(), ToolError> {
        if ev.data.is_empty() {
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(&ev.data).map_err(|e| ToolError::new(ErrorCode::GarbledOutput, format!("Anthropic sent an event we couldn't read: {e}")))?;
        let kind = v.get("type").and_then(Value::as_str).or(ev.event.as_deref()).unwrap_or_default();
        let index = v.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).unwrap_or(0);
        match kind {
            "message_start" => {
                let m = v.get("message").cloned().unwrap_or(Value::Null);
                self.model = m.get("model").and_then(Value::as_str).unwrap_or_default().to_string();
                if let Some(u) = m.get("usage") {
                    self.read_usage(u);
                }
            }
            "content_block_start" => {
                let b = v.get("content_block").cloned().unwrap_or(Value::Null);
                let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let p = match b.get("type").and_then(Value::as_str).unwrap_or_default() {
                    "text" => Partial::Text(s("text")),
                    "thinking" => Partial::Thinking(s("thinking"), s("signature")),
                    "redacted_thinking" => Partial::Redacted(s("data")),
                    "tool_use" | "server_tool_use" => Partial::Tool { id: s("id"), name: s("name"), json: String::new() },
                    _ => Partial::Empty,
                };
                if !s("text").is_empty() {
                    on_delta(Delta::Text(s("text")));
                }
                if let Some(slot) = self.slot(index) {
                    *slot = p;
                }
            }
            "content_block_delta" => {
                let d = v.get("delta").cloned().unwrap_or(Value::Null);
                let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let dtype = d.get("type").and_then(Value::as_str).unwrap_or_default().to_string();
                let Some(slot) = self.slot(index) else { return Ok(()) };
                match (dtype.as_str(), slot) {
                    ("text_delta", Partial::Text(t)) => {
                        let piece = s("text");
                        t.push_str(&piece);
                        on_delta(Delta::Text(piece));
                    }
                    ("text_delta", slot @ Partial::Empty) => {
                        let piece = s("text");
                        *slot = Partial::Text(piece.clone());
                        on_delta(Delta::Text(piece));
                    }
                    ("thinking_delta", Partial::Thinking(t, _)) => {
                        let piece = s("thinking");
                        t.push_str(&piece);
                        on_delta(Delta::Thinking(piece));
                    }
                    ("signature_delta", Partial::Thinking(_, sig)) => sig.push_str(&s("signature")),
                    ("input_json_delta", Partial::Tool { json, .. }) => {
                        json.push_str(&s("partial_json"));
                        if json.len() > 4 * 1024 * 1024 {
                            return Err(ToolError::new(ErrorCode::OverLimit, "The model sent oversized tool input."));
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" | "ping" => {}
            "message_delta" => {
                if let Some(r) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop = Some(StopReason::parse(r));
                }
                if let Some(u) = v.get("usage") {
                    self.read_usage(u);
                }
            }
            "message_stop" => self.done = true,
            "error" => {
                let et = v.pointer("/error/type").and_then(Value::as_str).unwrap_or("error");
                let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("unknown error");
                let code = match et {
                    "overloaded_error" | "api_error" => ErrorCode::EngineMissing,
                    "rate_limit_error" => ErrorCode::OverLimit,
                    "authentication_error" | "permission_error" => ErrorCode::NoPermission,
                    _ => ErrorCode::ProgramFailed,
                };
                return Err(ToolError::new(code, format!("Anthropic stream error ({et}): {msg}"))
                    .with_remediation("Try again; if it keeps happening, switch backend.")
                    .with_details(json!({"provider_error": et})));
            }
            _ => {}
        }
        Ok(())
    }

    fn read_usage(&mut self, u: &Value) {
        let g = |k: &str| u.get(k).and_then(Value::as_u64);
        if let Some(n) = g("input_tokens") {
            self.usage.input_tokens = n;
        }
        if let Some(n) = g("output_tokens") {
            self.usage.output_tokens = n;
        }
        if let Some(n) = g("cache_read_input_tokens") {
            self.usage.cache_read_tokens = n;
        }
        if let Some(n) = g("cache_creation_input_tokens") {
            self.usage.cache_write_tokens = n;
        }
    }

    /// The finished assistant message.
    pub fn finish(self, fallback_model: &str) -> Result<TurnResult, ToolError> {
        let mut blocks = Vec::new();
        for p in self.blocks {
            match p {
                Partial::Empty => {}
                Partial::Text(t) => blocks.push(Block::Text { text: t }),
                Partial::Thinking(t, s) => blocks.push(Block::Thinking { thinking: t, signature: s }),
                Partial::Redacted(d) => blocks.push(Block::RedactedThinking { data: d }),
                Partial::Tool { id, name, json } => {
                    // Eagerly streamed input is not validated by the server: a truncated or
                    // malformed object becomes a marked input the agent loop answers with an error.
                    let input = if json.trim().is_empty() { json!({}) } else { serde_json::from_str(&json).unwrap_or_else(|_| invalid_input(&json)) };
                    blocks.push(Block::ToolUse { id, name, input });
                }
            }
        }
        let stop = match self.stop {
            Some(s) => s,
            None if self.done => StopReason::EndTurn,
            None => return Err(ToolError::new(ErrorCode::Truncated, "The answer stream ended early.").with_remediation("Try again.")),
        };
        let model = if self.model.is_empty() { fallback_model.to_string() } else { self.model };
        Ok(TurnResult { message: Msg { role: Speaker::Assistant, blocks }, stop, usage: self.usage, model, follow_up: None })
    }
}

impl ChatModel for Anthropic {
    fn turn(&mut self, req: &TurnRequest<'_>, on_delta: &mut dyn FnMut(Delta), stop: &Stop) -> Result<TurnResult, ToolError> {
        self.learn();
        stop.check()?;
        let agent = http::stream_agent(self.stream_limit);
        let reader = http::post_json(&agent, "Anthropic", &self.url("messages"), &self.headers(), &self.body(req))?;
        let mut state = StreamState::default();
        http::parse_sse(reader, |ev| {
            stop.check()?;
            state.apply(&ev, on_delta)?;
            Ok(!state.done)
        })?;
        stop.check()?;
        state.finish(&self.model)
    }

    fn model(&self) -> String {
        self.model.clone()
    }
}

/// Marker for tool input that wasn't valid JSON.
pub const INVALID_INPUT_KEY: &str = "__invalid_json_input";

pub fn invalid_input(raw: &str) -> Value {
    json!({ INVALID_INPUT_KEY: raw.chars().take(200).collect::<String>() })
}

/// Tool definitions sorted by name so the cached prefix stays identical between requests.
pub fn sorted(mut tools: Vec<ToolDef>) -> Vec<ToolDef> {
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Effort;

    /// #1100: Anthropic's own address is told apart by its host, not by a look-alike.
    #[test]
    fn anthropics_own_address_is_recognised() {
        for yes in ["https://api.anthropic.com/v1", "https://api.anthropic.com", "HTTPS://API.Anthropic.com/v1/"] {
            assert!(is_anthropic_api(yes), "{yes}");
        }
        for no in ["http://api.anthropic.com/v1", "https://api.anthropic.com.example.test/v1", "https://proxy.test/api.anthropic.com/v1", "api.anthropic.com"] {
            assert!(!is_anthropic_api(no), "{no}");
        }
    }

    #[test]
    fn body_has_required_fields() {
        let a = Anthropic::new(None, "k".into(), "");
        let msgs = vec![Msg::user_text("hi")];
        let tools = vec![
            ToolDef { name: "a".into(), description: "d".into(), schema: json!({"type": "object"}) },
            ToolDef { name: "b".into(), description: "d".into(), schema: json!({"type": "object"}) },
        ];
        let b = a.body(&TurnRequest { system: "sys", messages: &msgs, tools: &tools, effort: Effort::High, max_tokens: MAX_TOKENS });
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["thinking"]["type"], "adaptive");
        assert_eq!(b["output_config"]["effort"], "high");
        assert_eq!(b["tool_choice"]["type"], "auto");
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["max_tokens"], 64000);
        assert!(b["tools"][0].get("cache_control").is_none());
        assert_eq!(b["tools"][1]["cache_control"]["type"], "ephemeral");
        assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
        assert!(a.headers().iter().any(|(k, v)| *k == "anthropic-beta" && v == FALLBACK_BETA));
        let mut off = a.clone();
        off.fallbacks = false;
        assert!(off.body(&TurnRequest { system: "s", messages: &msgs, tools: &[], effort: Effort::Low, max_tokens: 10 }).get("fallbacks").is_none());
        assert!(off.headers().iter().all(|(k, _)| *k != "anthropic-beta"));
        // The body never carries sampling settings (the 2026-10-21 API version rejects them).
        for key in ["temperature", "top_p", "top_k"] {
            assert!(b.get(key).is_none(), "{key}");
        }
    }

    /// The fallback beta is Anthropic's own: Bedrock, Vertex, proxies and gateways answer 400 to it.
    #[test]
    fn the_fallback_beta_goes_only_to_anthropics_own_api() {
        let msgs = vec![Msg::user_text("hi")];
        let req = TurnRequest { system: "s", messages: &msgs, tools: &[], effort: Effort::High, max_tokens: 10 };
        for (base, sent) in
            [(None, true), (Some("https://api.anthropic.com"), true), (Some("https://proxy.test/v1"), false), (Some("http://127.0.0.1:20128/v1"), false)]
        {
            let a = Anthropic::new(base, "sk-test".into(), "");
            assert!(a.fallbacks, "on by default");
            assert_eq!(a.body(&req).get("fallbacks").is_some(), sent, "{base:?}");
            assert_eq!(a.headers().iter().any(|(k, v)| *k == "anthropic-beta" && v == FALLBACK_BETA), sent, "{base:?}");
            assert_eq!(a.learn_capabilities, sent, "only Anthropic's own list is read before a turn");
        }
    }

    /// A model whose list entry says it takes no effort or thinking setting isn't sent one; one that
    /// lists its levels gets the nearest it has; unknown is sent as before.
    #[test]
    fn effort_and_thinking_follow_the_models_capabilities() {
        let base = "https://capabilities.test";
        let entry = |id: &str, caps: Value| model_entry(&json!({"id": id, "capabilities": caps})).unwrap();
        model_info::remember(
            base,
            vec![
                entry("plain", json!({"effort": {"supported": false}, "thinking": {"supported": true, "types": {"adaptive": {"supported": false}}}})),
                entry(
                    "three",
                    json!({"effort": {"supported": true, "low": {"supported": true}, "medium": {"supported": true}, "high": {"supported": true}, "max": {"supported": false}, "xhigh": null}}),
                ),
            ],
            model_info::LIST_TTL,
        );
        let req = |effort| TurnRequest { system: "s", messages: &[], tools: &[], effort, max_tokens: 10 };
        let b = Anthropic::new(Some(base), "k".into(), "plain").body(&req(Effort::High));
        assert!(b.get("output_config").is_none() && b.get("thinking").is_none(), "{b}");
        let b = Anthropic::new(Some(base), "k".into(), "three").body(&req(Effort::Max));
        assert_eq!(b["output_config"]["effort"], "high");
        assert_eq!(b["thinking"]["type"], "adaptive");
        let b = Anthropic::new(Some(base), "k".into(), "unlisted").body(&req(Effort::Xhigh));
        assert_eq!((b["output_config"]["effort"].as_str(), b["thinking"]["type"].as_str()), (Some("xhigh"), Some("adaptive")));
        let b = Anthropic::new(Some(base), "k".into(), "claude-sonnet-4-6").body(&req(Effort::Xhigh));
        assert_eq!(b["output_config"]["effort"], "high", "the offline table still applies to an unlisted model");
    }

    #[test]
    fn model_entries_are_read_and_hostile_values_are_unknown() {
        let full = json!({
            "id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "max_input_tokens": 1_000_000, "max_tokens": 128_000,
            "lifecycle": "active",
            "capabilities": {
                "image_input": {"supported": true},
                "effort": {"supported": true, "low": {"supported": true}, "medium": {"supported": true}, "high": {"supported": true}, "xhigh": {"supported": true}, "max": {"supported": true}},
                "thinking": {"supported": true, "types": {"adaptive": {"supported": true}}}
            }
        });
        let info = model_entry(&full).unwrap();
        assert_eq!(info.display_name.as_deref(), Some("Claude Opus 5.5"));
        assert_eq!((info.context_tokens, info.max_output_tokens), (Some(1_000_000), Some(128_000)));
        assert_eq!((info.vision, info.reasoning, info.deprecated), (Some(true), Some(true), false));
        assert_eq!(info.efforts.as_deref(), Some(&Effort::ALL[..]));
        for bad in [json!("x"), json!(-1), json!(1e300), json!(0), json!([1])] {
            let info = model_entry(&json!({"id": "m", "max_input_tokens": bad, "max_tokens": bad, "capabilities": bad, "display_name": bad, "lifecycle": bad}))
                .unwrap();
            let display_name = bad.as_str().map(str::to_string);
            assert_eq!(info, ModelInfo { id: "m".into(), display_name, ..Default::default() }, "{bad}");
        }
        assert!(model_entry(&json!({"id": 5})).is_none());
        assert!(model_entry(&json!({"id": ""})).is_none());
        assert!(model_entry(&json!("claude")).is_none());
        assert!(model_entry(&json!({"id": "old", "lifecycle": "deprecated"})).unwrap().deprecated);
        assert_eq!(model_entry(&json!({"id": "m", "capabilities": {"effort": {}}})).unwrap().efforts, None, "nothing said");
        assert_eq!(model_entry(&json!({"id": "m", "capabilities": {"thinking": {"supported": false}}})).unwrap().reasoning, Some(false));
        assert_eq!(query_value("a b&c=é"), "a%20b%26c%3D%C3%A9");
    }

    /// A stand-in `/v1/models` server: each request gets `answer(n)` for its number `n`, up to
    /// `max` requests; the paths asked for are kept.
    fn pages(max: usize, answer: impl Fn(usize) -> String + Send + 'static) -> (String, std::sync::Arc<craft_anything_platform::sync::Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let paths = std::sync::Arc::new(craft_anything_platform::sync::Mutex::new(Vec::new()));
        let seen = paths.clone();
        std::thread::spawn(move || {
            for n in 0..max {
                let Ok((mut stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                seen.lock().unwrap().push(line.split_whitespace().nth(1).unwrap_or_default().to_string());
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                }
                let body = answer(n);
                let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        (url, paths)
    }

    #[test]
    fn the_model_list_is_read_page_by_page() {
        let (url, paths) = pages(2, |n| match n {
            0 => json!({"data": [{"id": "a", "max_input_tokens": 1000}, {"id": "b"}], "has_more": true, "last_id": "b"}).to_string(),
            _ => json!({"data": [{"id": "c"}, {"id": "a"}], "has_more": false, "last_id": "c"}).to_string(),
        });
        let a = Anthropic::new(Some(&url), "sk-test".into(), "a");
        let models = a.list_model_info(Duration::from_secs(10)).unwrap();
        assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"], "joined, without the repeat");
        assert_eq!(models[0].context_tokens, Some(1000));
        assert_eq!(*paths.lock().unwrap(), ["/v1/models?limit=1000", "/v1/models?limit=1000&after_id=b"]);
        assert!(matches!(model_info::recall(&url, "a"), Known::Listed(Some(m)) if m.context_tokens == Some(1000)), "remembered for turns");
    }

    #[test]
    fn a_list_that_never_ends_stops_after_five_pages() {
        let (url, paths) = pages(20, |n| json!({"data": [{"id": format!("m{n}")}], "has_more": true, "last_id": format!("m{n}")}).to_string());
        let models = Anthropic::new(Some(&url), "sk-test".into(), "").list_models(Duration::from_secs(10)).unwrap();
        assert_eq!(models, ["m0", "m1", "m2", "m3", "m4"]);
        assert_eq!(paths.lock().unwrap().len(), MAX_MODEL_PAGES);
        // The same cursor again is not followed.
        let (url, paths) = pages(20, |_| json!({"data": [{"id": "same"}], "has_more": true, "last_id": "same"}).to_string());
        assert_eq!(Anthropic::new(Some(&url), "k".into(), "").list_models(Duration::from_secs(10)).unwrap(), ["same"]);
        assert_eq!(paths.lock().unwrap().len(), 2);
    }

    #[test]
    fn an_oversized_model_list_is_refused_not_read_into_memory() {
        let (url, _) = pages(1, |_| {
            let entries = vec![r#"{"id":"claude-model-with-a-long-name"}"#; 1_000_000].join(",");
            format!(r#"{{"data":[{entries}],"has_more":false}}"#)
        });
        let e = Anthropic::new(Some(&url), "k".into(), "").list_model_info(Duration::from_secs(20)).unwrap_err();
        assert_eq!(e.code, ErrorCode::OverLimit, "{e:?}");
    }

    #[test]
    fn hostile_list_pages_never_panic() {
        for body in ["{}", "[]", "null", r#"{"data": 5, "has_more": true}"#, r#"{"data": [null, 1, "x", {"id": null}], "has_more": "yes", "last_id": 7}"#] {
            let (url, _) = pages(1, move |_| body.to_string());
            assert_eq!(Anthropic::new(Some(&url), "k".into(), "").list_models(Duration::from_secs(10)).unwrap(), Vec::<String>::new(), "{body}");
        }
        let (url, _) = pages(1, |_| "not json".to_string());
        assert_eq!(Anthropic::new(Some(&url), "k".into(), "").list_models(Duration::from_secs(10)).unwrap_err().code, ErrorCode::GarbledOutput);
    }

    #[test]
    fn a_context_window_stop_is_typed() {
        let mut st = StreamState::default();
        for e in [
            json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":10}}}),
            json!({"type":"message_delta","delta":{"stop_reason":"model_context_window_exceeded"},"usage":{"output_tokens":3}}),
            json!({"type":"message_stop"}),
        ] {
            st.apply(&SseEvent { event: None, data: e.to_string() }, &mut |_| {}).unwrap();
        }
        let r = st.finish("m").unwrap();
        assert_eq!(r.stop, StopReason::ContextWindow);
        assert_eq!(r.stop.as_str(), "model_context_window_exceeded");
    }

    /// Every level goes to a model that has it; one that lacks it gets the nearest level below.
    #[test]
    fn effort_steps_down_to_a_level_the_model_has() {
        for e in Effort::ALL {
            assert_eq!(effort_for("claude-opus-5-5", e), e);
            assert_eq!(Effort::parse(&e.as_str().to_uppercase()), Some(e));
        }
        for model in ["claude-opus-5-5", "claude-sonnet-4-6", "anthropic/claude-future", "anthropic.claude-future"] {
            let client = Anthropic::new(None, "sk-test".into(), model);
            for effort in [Effort::None, Effort::Minimal] {
                assert_eq!(effort_for(model, effort), Effort::Low);
                let body = client.body(&TurnRequest { system: "s", messages: &[], tools: &[], effort, max_tokens: 10 });
                assert_eq!(body["output_config"]["effort"], "low");
                assert_eq!(Effort::parse(effort.as_str()), Some(effort));
            }
        }
        assert_eq!(effort_for("claude-sonnet-4-6", Effort::Xhigh), Effort::High);
        assert_eq!(effort_for("anthropic/claude-opus-4-6", Effort::Max), Effort::Max);
        assert_eq!(effort_for("claude-opus-4-5-20251101", Effort::Max), Effort::High);
        assert_eq!(Effort::Low.within(&[Effort::Medium, Effort::High]), Effort::Medium, "none below: the lowest it has");
        assert_eq!(Effort::Max.within(&[]), Effort::Max);
        assert_eq!(Effort::parse("extreme"), None);
        let a = Anthropic::new(None, "k".into(), "claude-sonnet-4-6");
        let b = a.body(&TurnRequest { system: "s", messages: &[], tools: &[], effort: Effort::Xhigh, max_tokens: 10 });
        assert_eq!(b["output_config"]["effort"], "high");
    }

    #[test]
    fn stream_state_builds_blocks() {
        let events = [
            json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":10,"cache_read_input_tokens":4}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"x","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"a\":"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"1}"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}),
            json!({"type":"message_stop"}),
        ];
        let mut st = StreamState::default();
        let mut thinking = String::new();
        for e in events {
            st.apply(&SseEvent { event: None, data: e.to_string() }, &mut |d| {
                if let Delta::Thinking(t) = d {
                    thinking.push_str(&t)
                }
            })
            .unwrap();
        }
        let r = st.finish("m").unwrap();
        assert_eq!(thinking, "hmm");
        assert_eq!(r.stop, StopReason::ToolUse);
        assert_eq!(r.usage, Usage { input_tokens: 10, output_tokens: 7, cache_read_tokens: 4, cache_write_tokens: 0 });
        assert_eq!(r.message.blocks[0], Block::Thinking { thinking: "hmm".into(), signature: "sig".into() });
        assert_eq!(r.message.tool_uses(), vec![("t1".into(), "x".into(), json!({"a": 1}))]);
    }

    #[test]
    fn stream_errors_and_truncation() {
        let mut st = StreamState::default();
        let e = st
            .apply(
                &SseEvent { event: Some("error".into()), data: json!({"type":"error","error":{"type":"overloaded_error","message":"busy"}}).to_string() },
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::EngineMissing);
        let st = StreamState::default();
        assert_eq!(st.finish("m").unwrap_err().code, ErrorCode::Truncated);
        let mut st = StreamState::default();
        assert!(st.apply(&SseEvent { event: None, data: "{not json".into() }, &mut |_| {}).is_err());
        // indexes far out of range are ignored, not allocated
        st.apply(
            &SseEvent { event: None, data: json!({"type":"content_block_start","index":999999999,"content_block":{"type":"text","text":""}}).to_string() },
            &mut |_| {},
        )
        .unwrap();
    }
}
