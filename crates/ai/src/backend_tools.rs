//! Setting up AI backends as tools, so the CLI, MCP clients and the agent can do what Settings → AI
//! does: list the providers an AI can be added from, add one with its key (checked first), change
//! a key, remove or test an AI, and look for AI on this computer. A key is never returned, and the
//! journal redacts `api_key` like every secret-named field.
//!
//! A tool runs while its call holds the session, so these read and save through `ctx.session`
//! ([`provider_setup`]) and never through [`crate::AiHub`]'s settings methods, which would lock it
//! again.

use crate::provider_setup::{self, key_store, save_in_session};
use crate::{AiConfig, AiHub, BackendKind, detect, offers};
use craft_anything_engine::EngineRef;
use craft_anything_engine::tool::{Ctx, Effects, Requires, ToolOutput, ToolSpec, Toolset, parse_params, schema_of};
use craft_anything_evidence::{ErrorCode, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

pub fn specs() -> Vec<ToolSpec> {
    let spec = |id, title, summary, beginner, effects, input_schema, run| ToolSpec {
        id,
        title,
        summary,
        beginner,
        toolset: Toolset::Setup,
        effects,
        requires: Requires::Nothing,
        input_schema,
        run,
    };
    let saves_and_checks = Effects::WRITES_FILESYSTEM.union(Effects::ACCESSES_NETWORK);
    vec![
        spec(
            "ai_providers_list",
            "List AI providers",
            "List the providers an AI can be added from with an API key (`ai_backend_add` with `offer`): id, name, how it's paid for (`billing`), the default and suggested models, where a key is made (`key_url`) and the environment variables that usually hold it. Ids are used as given (`opencode-go`). This is not what's found on this computer (`ai_backends_detect`).",
            "Shows the AI services you can add with a key.",
            Effects::READ_ONLY,
            schema_of::<NoParams>,
            ai_providers_list,
        ),
        spec(
            "ai_backends_list",
            "List the AIs set up",
            "List the AI backends set up (name, kind, model, address, on or off), whether each has a key stored (`key_stored`) and whether it's ready to use (`has_key`: stored, in its environment variable, or none needed), and the roles and fallback chain. Keys are never shown.",
            "Shows the AIs you've connected.",
            Effects::READ_ONLY,
            schema_of::<NoParams>,
            ai_backends_list,
        ),
        spec(
            "ai_backends_detect",
            "Find AI on this computer",
            "Look for AI ready to use here: API keys in the environment, local model servers (short loopback probes), coding agents on PATH and SSH hosts; at most a few seconds. AIs already set up are left out. To add a provider with a key, see `ai_providers_list`.",
            "Looks for AI you can use on this computer.",
            Effects::ACCESSES_NETWORK,
            schema_of::<NoParams>,
            ai_backends_detect,
        ),
        spec(
            "ai_backend_add",
            "Add an AI",
            "Add an AI backend, from a provider (`offer`, an id from `ai_providers_list`) or an OpenAI-compatible server's `address`. A name already set up is refused unless `replace` is true; a replaced backend keeps no old key unless a new one is given. Prefer `api_key_env` (the name of an environment variable holding the key, read where this runs) over `api_key`: a key pasted here is seen by whoever sends it. `api_key_env` takes a provider's own variable (see `ai_providers_list`) only for that provider at its own address, and otherwise only a name ending in `_API_KEY`. Unless `check` is false, the provider must list its models for the key first (at most 10 s); a rejected key or no answer saves nothing. Without `model` the provider's default is used when it lists it, else the first model that can chat. The key goes to the system keychain (or the private key file) under the backend's name and is never returned.",
            "Connects an AI with its key, after checking the key works.",
            saves_and_checks,
            schema_of::<AddParams>,
            ai_backend_add,
        ),
        spec(
            "ai_backend_key_set",
            "Change an AI's key",
            "Store a new API key for an AI backend that's set up. Prefer `api_key_env` over `api_key` (see `ai_backend_add`). Unless `check` is false, its provider must list its models for the new key first (at most 10 s); a rejected key keeps the old one. The key is never returned.",
            "Changes the key an AI uses.",
            saves_and_checks,
            schema_of::<KeyParams>,
            ai_backend_key_set,
        ),
        spec(
            "ai_backend_remove",
            "Remove an AI",
            "Remove an AI backend, its stored key, and its place in roles and the fallback chain. `removed` is false when there was no such backend; `key_removed` is false when the backend was removed but its stored key couldn't be (it's never used by another backend).",
            "Disconnects an AI and forgets its key.",
            Effects::WRITES_FILESYSTEM,
            schema_of::<NameParams>,
            ai_backend_remove,
        ),
        spec(
            "ai_backend_test",
            "Test an AI",
            "Send an AI backend a tiny request and time it (at most 60 s); a coding agent app only runs its `--version` (`version_only`). Uses the settings in use.",
            "Checks that an AI answers.",
            Effects::ACCESSES_NETWORK.union(Effects::LAUNCHES_PROCESS),
            schema_of::<NameParams>,
            ai_backend_test,
        ),
    ]
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameParams {
    /// The backend's name (see `ai_backends_list`).
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddParams {
    /// A provider's id from `ai_providers_list` (`anthropic`, `openai`, `groq`…). Give this or `address`.
    #[serde(default)]
    pub offer: Option<String>,
    /// An OpenAI-compatible server's address, like `http://192.168.1.20:11434/v1` (Anthropic's own is recognised). Give this or `offer`.
    #[serde(default)]
    pub address: Option<String>,
    /// The backend's name. Default with `offer`: the provider's usual name ("Claude"); needed with `address`.
    #[serde(default)]
    pub name: Option<String>,
    /// The API key. Prefer `api_key_env`.
    #[serde(default)]
    pub api_key: Option<String>,
    /// The name of an environment variable holding the key, read where this runs.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// The model to use (default: the provider's, or the first one listed that can chat).
    #[serde(default)]
    pub model: Option<String>,
    /// With `offer`: another address for the same API (a proxy).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Ask the provider for its models with the key before saving (default true). False saves
    /// without any request (offline set-up; with `address` a model must be named).
    #[serde(default)]
    pub check: Option<bool>,
    /// Replace a backend of the same name (default false: refused). Its stored key is dropped
    /// unless a new one is given.
    #[serde(default)]
    pub replace: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyParams {
    /// The backend's name (see `ai_backends_list`).
    pub name: String,
    /// The API key. Prefer `api_key_env`.
    #[serde(default)]
    pub api_key: Option<String>,
    /// The name of an environment variable holding the key, read where this runs.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Ask the provider for its models with the key before storing it (default true).
    #[serde(default)]
    pub check: Option<bool>,
}

/// The key given as `api_key` or read from `api_key_env`; never both. `own` is the provider whose
/// own address the key goes to, if any ([`provider_setup::key_from_env`]).
fn key_param(api_key: Option<String>, api_key_env: Option<&str>, own: Option<&offers::ProviderOffer>) -> Result<Option<String>, ToolError> {
    match (api_key, api_key_env.map(str::trim).filter(|v| !v.is_empty())) {
        (Some(_), Some(_)) => Err(ToolError::invalid("Give `api_key` or `api_key_env`, not both.")),
        (Some(k), None) => Ok(Some(k)),
        (None, Some(var)) => provider_setup::key_from_env(var, own).map(Some),
        (None, None) => Ok(None),
    }
}

fn no_backend(name: &str) -> ToolError {
    let shown: String = name.chars().take(100).collect();
    ToolError::invalid(format!("No AI backend named {shown:?}.")).with_remediation("See `ai_backends_list`.")
}

fn ai_providers_list(_ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let _: NoParams = parse_params(params)?;
    Ok(ToolOutput::Value(json!({"providers": offers::offers()})))
}

fn ai_backends_list(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let _: NoParams = parse_params(params)?;
    let cfg = AiConfig::from_table(&ctx.session.settings.ai)?;
    let keys = key_store(ctx.session);
    // Options are left out: an agent's arguments could hold a secret.
    let backends: Vec<Value> = cfg
        .backends
        .iter()
        .map(|b| {
            let c = &b.config;
            let stored = c.kind != BackendKind::AgentCli && keys.get(&c.name).is_some();
            json!({
                "name": c.name,
                "kind": c.kind,
                "model": c.model,
                "model_in_use": crate::model_in_use(c, Some(&b.options)),
                "base_url": c.base_url,
                "ssh_host": c.ssh_host,
                "enabled": c.enabled,
                "key_stored": stored,
                "has_key": stored || provider_setup::key_ready(b, &keys),
            })
        })
        .collect();
    Ok(ToolOutput::Value(json!({"backends": backends, "roles": cfg.roles, "fallback_chain": cfg.fallback_chain})))
}

fn ai_backends_detect(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let _: NoParams = parse_params(params)?;
    let set_up: Vec<crate::BackendConfig> = AiConfig::from_settings(&ctx.session.settings).backends.into_iter().map(|b| b.config).collect();
    let found = crate::leave_out_set_up(detect::detect_all(), &set_up);
    let hints: Vec<Value> = detect::hints().iter().map(|h| json!({"message": h.message(), "remediation": h.remediation()})).collect();
    Ok(ToolOutput::Value(json!({"found": found, "hints": hints})))
}

fn ai_backend_add(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: AddParams = parse_params(params)?;
    let blank = |s: &Option<String>| s.as_deref().is_none_or(|s| s.trim().is_empty());
    // Only an offer at its own address may read that provider's variable.
    let own = if blank(&p.address) && blank(&p.base_url) { p.offer.as_deref().and_then(offers::offer) } else { None };
    let key = key_param(p.api_key, p.api_key_env.as_deref(), own.as_ref())?;
    let check = p.check.unwrap_or(true);
    let prepared = match (p.offer.as_deref().filter(|_| !blank(&p.offer)), p.address.as_deref().filter(|_| !blank(&p.address))) {
        (Some(offer), None) => provider_setup::prepare_offer(offer, p.name.as_deref(), key, p.model.as_deref(), p.base_url.as_deref(), check)?,
        (None, Some(address)) => {
            if !blank(&p.base_url) {
                return Err(ToolError::invalid("`base_url` goes with `offer`; give a server's address as `address`."));
            }
            let name = p.name.as_deref().filter(|n| !n.trim().is_empty()).ok_or_else(|| ToolError::invalid("Name the AI (`name`)."))?;
            provider_setup::prepare_server(name, address, p.model.as_deref(), key, check)?
        }
        _ => return Err(ToolError::invalid("Give `offer` (an id from `ai_providers_list`) or `address`, one of them.")),
    };
    let cfg = AiConfig::from_table(&ctx.session.settings.ai)?;
    let keys = key_store(ctx.session);
    let added = provider_setup::save_added(prepared, cfg, &keys, p.replace.unwrap_or(false), |cfg| save_in_session(ctx.session, cfg))?;
    Ok(ToolOutput::Value(serde_json::to_value(added).map_err(|e| ToolError::internal(e.to_string()))?))
}

fn ai_backend_key_set(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: KeyParams = parse_params(params)?;
    let check = p.check.unwrap_or(true);
    let cfg = AiConfig::from_table(&ctx.session.settings.ai)?;
    let entry = cfg.get(&p.name).ok_or_else(|| no_backend(&p.name))?;
    let own = provider_setup::own_offer(entry.config.kind, entry.config.base_url.as_deref())
        .filter(|_| entry.config.ssh_host.as_deref().is_none_or(|h| h.trim().is_empty()));
    let key = key_param(p.api_key, p.api_key_env.as_deref(), own.as_ref())?
        .ok_or_else(|| ToolError::invalid("Give the key as `api_key_env` (preferred) or `api_key`."))?;
    let key = provider_setup::prepare_key(entry, &key, check)?;
    let stored_in = key_store(ctx.session).set(&entry.config.name, &key)?;
    Ok(ToolOutput::Value(json!({"name": entry.config.name, "key_stored": true, "key_stored_in": stored_in, "checked": check})))
}

fn ai_backend_remove(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: NameParams = parse_params(params)?;
    let mut cfg = AiConfig::from_table(&ctx.session.settings.ai)?;
    if cfg.get(&p.name).is_none() {
        return Ok(ToolOutput::Value(json!({"name": p.name, "removed": false})));
    }
    // The settings first: a key left behind is harmless (a backend added later under the name never
    // inherits it), a backend left without its key isn't.
    cfg.remove(&p.name);
    save_in_session(ctx.session, &cfg)?;
    Ok(ToolOutput::Value(match key_store(ctx.session).delete(&p.name) {
        Ok(()) => json!({"name": p.name, "removed": true, "key_removed": true}),
        Err(e) => json!({"name": p.name, "removed": true, "key_removed": false, "key_error": e.message}),
    }))
}

fn ai_backend_test(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: NameParams = parse_params(params)?;
    let entry = AiConfig::from_table(&ctx.session.settings.ai)?.get(&p.name).cloned().ok_or_else(|| no_backend(&p.name))?;
    let engine = ctx
        .session
        .ext
        .get::<EngineRef>()
        .and_then(|r| r.0.upgrade())
        .ok_or_else(|| ToolError::new(ErrorCode::FeatureMissing, "Testing an AI isn't available in this engine."))?;
    // The session is held by this call, so the entry comes from it and the hub never reads it.
    let hub = AiHub::with_stores(engine, ctx.session.config_dir().join("config.toml"), key_store(ctx.session));
    let report = hub.test_entry(&entry)?;
    Ok(ToolOutput::Value(serde_json::to_value(report).map_err(|e| ToolError::internal(e.to_string()))?))
}
