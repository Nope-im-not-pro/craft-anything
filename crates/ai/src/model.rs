//! Provider-neutral chat types the agent loop speaks; each backend maps them to its wire format.

use craft_anything_evidence::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// One piece of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    /// Model reasoning. Kept verbatim (with its signature) and replayed on tool turns.
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// What the server attached to the tool call `tool_use_id`, sent back with it unchanged
    /// (Gemini's thought signature: a call without it is refused).
    ToolExtra {
        tool_use_id: String,
        extra: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Msg {
    pub role: Speaker,
    pub blocks: Vec<Block>,
}

impl Msg {
    pub fn user_text(t: impl Into<String>) -> Self {
        Msg { role: Speaker::User, blocks: vec![Block::Text { text: t.into() }] }
    }

    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn tool_uses(&self) -> Vec<(String, String, Value)> {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::ToolUse { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
                _ => None,
            })
            .collect()
    }
}

/// A tool as offered to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// Shared thinking levels for API backends and agent CLI variants, ordered least to most.
/// Claude maps unsupported levels down with [`Effort::within`]; OpenCode requires the exact
/// variant in its live catalogue. The `effort` and `cli_thinking` settings remain independent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    None,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    /// Longer-horizon work than `high`.
    Xhigh,
    /// The deepest reasoning, with no limit on tokens spent.
    Max,
}

impl Effort {
    /// The API/Claude levels, least to most.
    pub const ALL: [Effort; 5] = [Effort::Low, Effort::Medium, Effort::High, Effort::Xhigh, Effort::Max];

    pub const CLI_ALL: [Effort; 7] = [Effort::None, Effort::Minimal, Effort::Low, Effort::Medium, Effort::High, Effort::Xhigh, Effort::Max];

    /// The stable lowercase id used in settings, API requests and agent model references.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::None => "none",
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }

    /// Parse a known level, ignoring surrounding whitespace and case.
    pub fn parse(s: &str) -> Option<Effort> {
        let s = s.trim();
        Effort::CLI_ALL.into_iter().find(|e| e.as_str().eq_ignore_ascii_case(s))
    }

    /// The display label for this level in model pickers and validation messages.
    pub fn title(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Xhigh => "Xhigh",
            Self::Max => "Max",
        }
    }

    /// This level on a model that offers only `levels`: the nearest one at or below it (as Claude
    /// Code does), else the lowest it has. Unchanged when `levels` is empty.
    pub fn within(self, levels: &[Effort]) -> Effort {
        levels.iter().copied().filter(|l| *l <= self).max().or_else(|| levels.iter().copied().min()).unwrap_or(self)
    }
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
    PauseTurn,
    StopSequence,
    /// The model filled its context window while answering (Claude's `model_context_window_exceeded`).
    ContextWindow,
    Other(String),
}

impl StopReason {
    pub fn parse(s: &str) -> StopReason {
        match s {
            "end_turn" | "stop" => StopReason::EndTurn,
            "tool_use" | "tool_calls" | "function_call" => StopReason::ToolUse,
            "max_tokens" | "length" => StopReason::MaxTokens,
            "refusal" | "content_filter" => StopReason::Refusal,
            "pause_turn" => StopReason::PauseTurn,
            "stop_sequence" => StopReason::StopSequence,
            "model_context_window_exceeded" => StopReason::ContextWindow,
            other => StopReason::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            StopReason::EndTurn => "end_turn",
            StopReason::ToolUse => "tool_use",
            StopReason::MaxTokens => "max_tokens",
            StopReason::Refusal => "refusal",
            StopReason::PauseTurn => "pause_turn",
            StopReason::StopSequence => "stop_sequence",
            StopReason::ContextWindow => "model_context_window_exceeded",
            StopReason::Other(s) => s,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, o: Usage) {
        self.input_tokens = self.input_tokens.saturating_add(o.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(o.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(o.cache_read_tokens);
        self.cache_write_tokens = self.cache_write_tokens.saturating_add(o.cache_write_tokens);
    }
}

/// One request to a chat backend.
#[derive(Debug, Clone)]
pub struct TurnRequest<'a> {
    pub system: &'a str,
    pub messages: &'a [Msg],
    pub tools: &'a [ToolDef],
    pub effort: Effort,
    pub max_tokens: u32,
}

/// One response.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnResult {
    pub message: Msg,
    pub stop: StopReason,
    pub usage: Usage,
    pub model: String,
    /// A provider-specific instruction to send to the model before continuing.
    pub follow_up: Option<String>,
}

/// Streaming pieces while a turn is generated.
#[derive(Debug, Clone, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
}

/// Cooperative cancellation for a conversation.
#[derive(Debug, Clone, Default)]
pub struct Stop {
    stopped: Arc<AtomicBool>,
    tool: Option<craft_anything_engine::CancelToken>,
}

impl Stop {
    /// Link discovery to cancellation of the engine tool that owns it.
    pub fn for_tool(cancel: craft_anything_engine::CancelToken) -> Self {
        Self { tool: Some(cancel), ..Default::default() }
    }

    /// Request cancellation for every clone sharing this conversation's stop flag.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    /// Clear a previous stop (at the start of the next message).
    pub fn reset(&self) {
        self.stopped.store(false, Ordering::SeqCst);
    }

    /// Whether the conversation or its owning engine tool has requested cancellation.
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst) || self.tool.as_ref().is_some_and(|c| c.is_cancelled())
    }

    pub fn check(&self) -> Result<(), ToolError> {
        if self.is_stopped() { Err(ToolError::cancelled()) } else { Ok(()) }
    }
}

/// A model that can take one turn. Implemented by the HTTP backends (and fakes in tests).
pub trait ChatModel: Send {
    fn turn(&mut self, req: &TurnRequest<'_>, on_delta: &mut dyn FnMut(Delta), stop: &Stop) -> Result<TurnResult, ToolError>;
    /// Model id, for display, journal and pricing.
    fn model(&self) -> String;
}
