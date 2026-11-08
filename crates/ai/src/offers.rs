//! The providers an AI can be added from with a key: the "Add a provider" list. This is kept apart
//! from what Detect finds on this computer ([`crate::detect`]): an offer is shown only when someone
//! asks to add a provider, never as something found, so a paid service is still never advertised
//! to someone who has no key for it.

use crate::{BackendKind, anthropic, detect, openai, providers};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a provider is paid for, as people see it before choosing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Billing {
    /// Each request is billed to the key's account.
    PayPerUse,
    /// The key is free to get (with the provider's own limits).
    FreeKey,
    /// A monthly plan covers it.
    Subscription,
    /// It runs on this computer and costs nothing per request.
    OnYourComputer,
    Unknown,
}

/// One provider an AI can be added from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderOffer {
    /// Stable, lower-case id: `anthropic`, `openai`, `groq`…
    pub id: String,
    /// What people see: "Anthropic (Claude API)".
    pub name: String,
    /// The name a new backend gets unless one is given (the same name Detect uses for it).
    pub default_name: String,
    pub kind: BackendKind,
    /// The address saved with the backend; `None` uses the kind's own.
    pub base_url: Option<String>,
    pub default_model: String,
    pub suggested_models: Vec<String>,
    pub billing: Billing,
    /// Where a key is made.
    pub key_url: Option<String>,
    /// Environment variables that conventionally hold its key, in the order they're tried.
    pub key_env: Vec<String>,
    pub needs_key: bool,
    /// A short line to show with the offer.
    pub note: Option<String>,
}

/// Where each named hosted provider's keys are made, where we know it: (provider name, id, key page).
const HOSTED: &[(&str, &str, Option<&str>)] = &[
    ("Moonshot", "moonshot", Some("https://platform.moonshot.ai/console/api-keys")),
    ("DeepSeek", "deepseek", Some("https://platform.deepseek.com/api_keys")),
    ("xAI (Grok)", "xai", Some("https://console.x.ai")),
    ("Groq", "groq", Some("https://console.groq.com/keys")),
    ("Mistral", "mistral", Some("https://console.mistral.ai/api-keys")),
    ("Together", "together", None),
    ("Fireworks", "fireworks", None),
    ("DeepInfra", "deepinfra", None),
];

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Every provider an AI can be added from with a key, in the order they're shown.
pub fn offers() -> Vec<ProviderOffer> {
    let row = |id: &str, name: &str, default_name: &str, kind, models: &[&str], billing, key_url: &str, key_env: &[&str]| ProviderOffer {
        id: id.to_string(),
        name: name.to_string(),
        default_name: default_name.to_string(),
        kind,
        base_url: None,
        default_model: models.first().map(|m| m.to_string()).unwrap_or_default(),
        suggested_models: strings(models),
        billing,
        key_url: Some(key_url.to_string()),
        key_env: strings(key_env),
        needs_key: true,
        note: None,
    };
    let mut out = vec![
        ProviderOffer {
            note: Some("Have a Claude Pro or Max plan? Use Claude Code instead (Harnesses tab).".into()),
            ..row(
                "anthropic",
                BackendKind::Anthropic.title(),
                "Claude",
                BackendKind::Anthropic,
                anthropic::MODELS,
                Billing::PayPerUse,
                "https://console.anthropic.com/settings/keys",
                &["ANTHROPIC_API_KEY"],
            )
        },
        row(
            "openai",
            BackendKind::Openai.title(),
            "OpenAI",
            BackendKind::Openai,
            &[openai::OPENAI_DEFAULT_MODEL],
            Billing::PayPerUse,
            "https://platform.openai.com/api-keys",
            &["OPENAI_API_KEY"],
        ),
        row(
            "openrouter",
            BackendKind::Openrouter.title(),
            "OpenRouter",
            BackendKind::Openrouter,
            openai::OPENROUTER_MODELS,
            Billing::PayPerUse,
            "https://openrouter.ai/keys",
            &["OPENROUTER_API_KEY"],
        ),
        row(
            "gemini",
            BackendKind::Gemini.title(),
            detect::GEMINI_NAME,
            BackendKind::Gemini,
            openai::GEMINI_MODELS,
            Billing::FreeKey,
            "https://aistudio.google.com/apikey",
            &["GEMINI_API_KEY"],
        ),
    ];
    for p in providers::PROVIDERS {
        let (id, key_url) =
            HOSTED.iter().find(|(name, _, _)| *name == p.name).map_or_else(|| (p.catalog_id.to_string(), None), |(_, id, url)| (id.to_string(), *url));
        out.push(ProviderOffer {
            id,
            name: p.name.to_string(),
            default_name: p.name.to_string(),
            kind: p.kind,
            base_url: Some(p.base_url.to_string()),
            default_model: p.default_model.to_string(),
            suggested_models: vec![p.default_model.to_string()],
            billing: Billing::PayPerUse,
            key_url: key_url.map(str::to_string),
            key_env: p.key_names().map(str::to_string).collect(),
            needs_key: true,
            note: None,
        });
    }
    out.push(ProviderOffer {
        id: "opencode-go".into(),
        name: "OpenCode Go".into(),
        default_name: detect::OPENCODE_GO_NAME.into(),
        kind: BackendKind::OpenaiCompatible,
        base_url: Some(openai::OPENCODE_GO_BASE.into()),
        default_model: openai::OPENCODE_GO_DEFAULT_MODEL.into(),
        suggested_models: vec![openai::OPENCODE_GO_DEFAULT_MODEL.into()],
        billing: Billing::Subscription,
        key_url: None,
        key_env: strings(&["OPENCODEGO_API_KEY", "OPENCODE_GO_API_KEY"]),
        needs_key: true,
        note: None,
    });
    out
}

/// The offer with this id (ids are compared without case).
pub fn offer(id: &str) -> Option<ProviderOffer> {
    let id = id.trim();
    offers().into_iter().find(|o| o.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offer_is_complete_and_has_its_own_id() {
        let all = offers();
        for o in &all {
            assert!(!o.id.is_empty() && o.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{}", o.id);
            assert!(!o.name.trim().is_empty() && !o.default_name.trim().is_empty() && !o.default_model.trim().is_empty(), "{}", o.id);
            assert!(o.suggested_models.contains(&o.default_model), "{}", o.id);
            assert!(o.base_url.as_deref().is_none_or(|b| b.starts_with("https://")), "{} is not https", o.id);
            assert!(o.key_url.as_deref().is_none_or(|u| u.starts_with("https://")), "{}", o.id);
            assert!(!o.key_env.is_empty(), "{}", o.id);
            assert_ne!(o.kind, BackendKind::AgentCli);
        }
        let mut ids: Vec<&str> = all.iter().map(|o| o.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "two offers share an id");
        assert_eq!(all.len(), 4 + providers::PROVIDERS.len() + 1);
    }

    #[test]
    fn the_claude_api_is_offered_with_a_key_and_where_to_get_one() {
        let c = offer("Anthropic").unwrap();
        assert_eq!((c.kind, c.billing, c.needs_key, c.default_name.as_str()), (BackendKind::Anthropic, Billing::PayPerUse, true, "Claude"));
        assert_eq!(c.key_url.as_deref(), Some("https://console.anthropic.com/settings/keys"));
        assert!(c.note.unwrap().contains("Claude Code"));
        assert_eq!(serde_json::to_value(Billing::OnYourComputer).unwrap(), "on_your_computer");
        assert!(offer("nope").is_none());
    }

    /// Offers never become "found": without a key in the environment, Detect still doesn't list
    /// the Claude API (D7: Gemini and Kimi stay where they were).
    #[test]
    fn an_offer_is_never_found_without_a_key() {
        let found = detect::env_keys(|_| None);
        assert!(found.iter().all(|d| d.suggestion.kind != BackendKind::Anthropic), "{found:?}");
        assert!(found.iter().any(|d| d.suggestion.name == detect::GEMINI_NAME));
        assert!(found.iter().any(|d| d.suggestion.name == detect::KIMI_NAME));
        assert!(found.iter().all(|d| !providers::PROVIDERS.iter().any(|p| p.name == d.suggestion.name)));
    }
}
