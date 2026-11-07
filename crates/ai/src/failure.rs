//! A failed turn's retry policy, kept apart from the words shown to the person. Protocol and
//! process boundaries supply these facts; display text never decides whether to retry.

use craft_anything_evidence::{ErrorCode, ToolError};

/// Why a conversation's last turn failed. Unknown failures need attention: repeating an
/// unclassified setup error can spend quota without doing any work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Authentication,
    Billing,
    MissingProgram,
    ToolsUnavailable,
    Configuration,
    Transient,
    RateLimited,
    Unknown,
}

/// A typed failure, and a provider's optional wait in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnFailure {
    pub kind: FailureKind,
    pub retry_after_secs: Option<u64>,
}

/// No failed turn waits more than fifteen minutes before its next attempt.
pub(crate) const MAX_RETRY_WAIT: u64 = 15 * 60;
pub(crate) const CONTINUE_TURN: &str =
    "Your last turn was cut off. Carry on from where you stopped: look at what's already in app/ and your checklist, then finish the task.";

impl TurnFailure {
    pub const fn new(kind: FailureKind) -> Self {
        Self { kind, retry_after_secs: None }
    }

    pub fn retryable(self) -> bool {
        matches!(self.kind, FailureKind::Transient | FailureKind::RateLimited)
    }

    pub(crate) fn from_api(error: &ToolError) -> Self {
        let status = error.details.as_ref().and_then(|d| d.get("status")).and_then(serde_json::Value::as_u64);
        let provider = error.details.as_ref().and_then(|d| d.get("provider_error")).and_then(serde_json::Value::as_str);
        let provider_code = error.details.as_ref().and_then(|d| d.get("provider_error_code")).and_then(serde_json::Value::as_str);
        let kind = match (status, provider) {
            // A spending cap answers 429 like a rate limit, but waiting doesn't lift it.
            (Some(429), _) if provider_code == Some(crate::http::SPEND_LIMIT) => FailureKind::Billing,
            (Some(401 | 403), _) | (_, Some("authentication_error" | "permission_error" | "invalid_api_key")) => FailureKind::Authentication,
            (Some(402), _) | (_, Some("billing_error" | "insufficient_quota")) => FailureKind::Billing,
            (Some(429), _) | (_, Some("rate_limit_error" | "rate_limit_exceeded")) => FailureKind::RateLimited,
            (Some(408 | 500..=599), _) | (_, Some("overloaded_error" | "api_error")) => FailureKind::Transient,
            (Some(400..=499), _) => FailureKind::Configuration,
            (_, Some(_)) => FailureKind::Unknown,
            _ => match error.code {
                ErrorCode::TimedOut | ErrorCode::EngineMissing | ErrorCode::Truncated => FailureKind::Transient,
                ErrorCode::NoPermission => FailureKind::Authentication,
                ErrorCode::BadInput | ErrorCode::BadSettings | ErrorCode::NotSupported | ErrorCode::FeatureMissing => FailureKind::Configuration,
                _ => FailureKind::Unknown,
            },
        };
        let retry_after_secs =
            error.details.as_ref().and_then(|d| d.get("retry_after_secs")).and_then(serde_json::Value::as_u64).map(|n| n.clamp(1, MAX_RETRY_WAIT));
        Self { kind, retry_after_secs }
    }

    pub(crate) fn from_process(error: &ToolError) -> Self {
        Self::new(match error.code {
            ErrorCode::EngineMissing | ErrorCode::NotFound => FailureKind::MissingProgram,
            ErrorCode::TimedOut => FailureKind::Transient,
            ErrorCode::NoPermission | ErrorCode::BadSettings | ErrorCode::BadInput => FailureKind::Configuration,
            _ => FailureKind::Unknown,
        })
    }

    /// Claude Code's documented `assistant.error` values, not the assistant's display text.
    pub(crate) fn claude(code: &str) -> Self {
        Self::new(match code {
            "authentication_failed" | "oauth_org_not_allowed" | "cloud_credential_error" => FailureKind::Authentication,
            "billing_error" | "account_on_hold" => FailureKind::Billing,
            "invalid_request" | "model_not_found" => FailureKind::Configuration,
            "rate_limit" => FailureKind::RateLimited,
            "server_error" | "overloaded" => FailureKind::Transient,
            _ => FailureKind::Unknown,
        })
    }

    /// Without a protocol error, a process may have crashed or lost its connection. Signals
    /// and nonzero exits keep the same bounded retry policy; a successful exit never opts in.
    pub(crate) fn process_exit(code: Option<i32>) -> Self {
        Self::new(if code == Some(0) { FailureKind::Unknown } else { FailureKind::Transient })
    }
}

/// Compatibility for a provider's display-only wait hint, after a typed status already made
/// the failure retryable. This never decides whether to retry. The number is bounded even when
/// an error asks for hours, or sends infinity or a negative delay.
pub(crate) fn retry_after(message: &str) -> Option<u64> {
    let m = message.to_ascii_lowercase();
    let tail = ["retry in ", "retry after ", "try again in "].iter().find_map(|s| m.find(s).and_then(|at| m.get(at + s.len()..)))?;
    let number: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n: f64 = number.parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    let unit = tail.get(number.len()..)?.trim_start();
    let scale = if unit.starts_with("ms") || unit.starts_with("millisecond") {
        0.001
    } else if unit.starts_with("min") || unit.starts_with('m') {
        60.0
    } else if unit.starts_with('h') {
        3600.0
    } else {
        1.0
    };
    Some((n * scale).ceil().clamp(1.0, MAX_RETRY_WAIT as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typed_failures_do_not_depend_on_their_display_language() {
        for message in ["请更新凭证", "The provider is unavailable", "anything"] {
            let auth = ToolError::new(ErrorCode::NoPermission, message).with_details(json!({"status":401}));
            assert_eq!(TurnFailure::from_api(&auth).kind, FailureKind::Authentication);
            assert!(!TurnFailure::from_api(&auth).retryable());
            let bill = ToolError::new(ErrorCode::BadInput, message).with_details(json!({"status":402}));
            assert_eq!(TurnFailure::from_api(&bill).kind, FailureKind::Billing);
            let absent = ToolError::new(ErrorCode::EngineMissing, message);
            assert_eq!(TurnFailure::from_process(&absent).kind, FailureKind::MissingProgram);
            assert!(!TurnFailure::from_process(&absent).retryable());
        }
        let transient = ToolError::new(ErrorCode::TimedOut, "The authentication service is restarting; billing and login are temporarily unavailable.");
        assert!(TurnFailure::from_api(&transient).retryable(), "English words don't override a typed timeout");
        let limit = ToolError::new(ErrorCode::OverLimit, "provider response").with_details(json!({"status":429,"retry_after_secs":2}));
        assert_eq!(TurnFailure::from_api(&limit), TurnFailure { kind: FailureKind::RateLimited, retry_after_secs: Some(2) });
        let quota = crate::http::status_error("Provider", 429, r#"{"error":{"code":"insufficient_quota","message":"余额不足"}}"#);
        assert_eq!(TurnFailure::from_api(&quota).kind, FailureKind::Billing);
        assert!(!TurnFailure::from_api(&ToolError::new(ErrorCode::OverLimit, "oversized input")).retryable(), "a local size limit is not rate limiting");
    }

    /// Anthropic's spend-cap 429 has no retry-after and doesn't lift by waiting: it is billing.
    #[test]
    fn a_spend_limit_429_is_billing_and_not_retried() {
        let body = json!({"type": "error", "error": {"type": "rate_limit_error", "message": "spend limit", "details": {"error_code": "enforced_spend_limit_reached"}}});
        let error = crate::http::status_error("Anthropic", 429, &body.to_string());
        assert_eq!(error.details.as_ref().unwrap()["provider_error_code"], "enforced_spend_limit_reached");
        assert!(error.remediation.as_deref().is_some_and(|r| r.contains("spending limit")), "{error:?}");
        let failure = TurnFailure::from_api(&error);
        assert_eq!(failure.kind, FailureKind::Billing);
        assert!(!failure.retryable());
        let rate = crate::http::status_error("Anthropic", 429, r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#);
        assert!(rate.details.as_ref().unwrap()["provider_error_code"].is_null());
        assert_eq!(TurnFailure::from_api(&rate).kind, FailureKind::RateLimited);
        for odd in [json!({"error": {"details": {"error_code": 7}}}), json!({"error": {"details": "x"}}), json!([1, 2])] {
            let e = crate::http::status_error("Anthropic", 429, &odd.to_string());
            assert_eq!(TurnFailure::from_api(&e).kind, FailureKind::RateLimited, "{odd}");
        }
    }

    #[test]
    fn streamed_provider_errors_keep_their_type() {
        for (provider, kind) in [
            ("rate_limit_exceeded", FailureKind::RateLimited),
            ("invalid_api_key", FailureKind::Authentication),
            ("insufficient_quota", FailureKind::Billing),
            ("unknown", FailureKind::Unknown),
        ] {
            let mut stream = crate::openai::ChatStream::default();
            let event = crate::http::SseEvent { event: None, data: json!({"error":{"code":provider,"message":"服务返回错误"}}).to_string() };
            let error = stream.apply(&event, &mut |_| {}).unwrap_err();
            assert_eq!(TurnFailure::from_api(&error).kind, kind);
        }
    }

    #[test]
    fn structured_claude_errors_and_plain_waits_have_bounded_retry_advice() {
        assert!(!TurnFailure::claude("billing_error").retryable());
        assert!(!TurnFailure::claude("authentication_failed").retryable());
        assert!(TurnFailure::claude("server_error").retryable());
        assert!(TurnFailure::claude("rate_limit").retryable());
        assert_eq!(retry_after("Please retry in 19.052906498s."), Some(20));
        assert_eq!(retry_after("Rate limit reached. Retry after 2 minutes"), Some(120));
        assert_eq!(retry_after("try again in 500ms"), Some(1));
        assert_eq!(retry_after("Retry in 45 seconds"), Some(45));
        for code in [None, Some(1), Some(2), Some(255)] {
            assert!(TurnFailure::process_exit(code).retryable());
        }
        assert!(!TurnFailure::process_exit(Some(0)).retryable());
        assert_eq!(retry_after("retry after 3 h"), Some(MAX_RETRY_WAIT));
        for text in ["retry after NaN", "retry after inf", "retry after -1s", "invalid command", "please retry in a moment", "retry in ."] {
            assert_eq!(retry_after(text), None);
        }
    }
}
