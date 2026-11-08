//! Adding an AI from a provider offer or by its address, and changing a saved AI's key: the checks
//! and what gets saved, apart from where the settings live. [`crate::AiHub`] reads and saves through
//! its own settings; the tools ([`crate::backend_tools`]) through the session they already hold, so
//! nothing here ever locks the session.

use crate::offers::{self, ProviderOffer};
use crate::{AiConfig, BackendConfig, BackendEntry, BackendKind, BackendOptions, KeyStore, anthropic, detect, openai, providers};
use craft_anything_engine::Session;
use craft_anything_evidence::{ErrorCode, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

/// How long a key check (listing the provider's models) may take.
pub const CHECK_TIME: Duration = Duration::from_secs(10);
/// The longest key we take.
pub const MAX_KEY_LEN: usize = 512;
const MAX_NAME_LEN: usize = 100;
const MAX_MODEL_LEN: usize = 256;
const MAX_ADDRESS_LEN: usize = 2048;

/// An AI that was added. The key is never part of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddedBackend {
    pub name: String,
    /// The provider offer it was added from, if any.
    pub offer: Option<String>,
    pub kind: BackendKind,
    pub model: String,
    pub base_url: Option<String>,
    /// A key was stored with it (in the system keychain or the private key file).
    pub key_stored: bool,
    /// Where the key went: `keychain` or `file`.
    pub key_stored_in: Option<String>,
    /// The provider was asked for its models with the key before saving.
    pub checked: bool,
    /// How many models it listed (0 when not checked).
    pub models_listed: usize,
}

/// An AI ready to save: checked (unless asked not to), nothing saved yet.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub entry: BackendEntry,
    pub key: Option<String>,
    pub offer: Option<String>,
    pub checked: bool,
    pub models_listed: usize,
}

/// A key is printable ASCII with no spaces or line breaks, at most [`MAX_KEY_LEN`] long. Checked
/// before anything is read, stored or sent; the key itself is never in the error.
pub fn check_key(key: &str) -> Result<(), ToolError> {
    let fix = "Copy the whole key again from the provider's page.";
    if key.is_empty() {
        return Err(ToolError::invalid("The key is empty.").with_remediation(fix));
    }
    if key.len() > MAX_KEY_LEN {
        return Err(ToolError::invalid(format!("The key is longer than {MAX_KEY_LEN} characters, so it isn't an API key.")).with_remediation(fix));
    }
    if !key.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(
            ToolError::invalid("An API key has only letters, digits and punctuation: no spaces, line breaks or other characters.").with_remediation(fix)
        );
    }
    Ok(())
}

fn check_name(name: &str) -> Result<String, ToolError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ToolError::invalid("A backend needs a name."));
    }
    if name.chars().count() > MAX_NAME_LEN || name.chars().any(char::is_control) {
        return Err(ToolError::invalid(format!("A backend's name is one line of at most {MAX_NAME_LEN} characters.")));
    }
    Ok(name.to_string())
}

fn check_model(model: Option<&str>) -> Result<Option<String>, ToolError> {
    match model.map(str::trim).filter(|m| !m.is_empty()) {
        None => Ok(None),
        Some(m) if m.chars().count() > MAX_MODEL_LEN || m.chars().any(char::is_control) => {
            Err(ToolError::invalid(format!("A model's name is one line of at most {MAX_MODEL_LEN} characters.")))
        }
        Some(m) => Ok(Some(m.to_string())),
    }
}

fn check_address(address: &str) -> Result<String, ToolError> {
    let address = address.trim().trim_end_matches('/');
    let lower = address.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://"))
        || address.len() > MAX_ADDRESS_LEN
        || address.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        let shown: String = address.chars().take(80).collect();
        return Err(
            ToolError::invalid(format!("{shown:?} isn't a web address.")).with_remediation("Give the server's address, like http://192.168.1.20:11434/v1.")
        );
    }
    Ok(address.to_string())
}

/// `e` with every copy of `key` taken out of its text and details.
pub(crate) fn scrub(mut e: ToolError, key: &str) -> ToolError {
    if key.is_empty() {
        return e;
    }
    const HIDDEN: &str = "[key hidden]";
    e.message = e.message.replace(key, HIDDEN);
    e.remediation = e.remediation.map(|r| r.replace(key, HIDDEN));
    e.details = e.details.and_then(|d| {
        let text = d.to_string();
        if text.contains(key) { serde_json::from_str(&text.replace(key, HIDDEN)).ok() } else { Some(d) }
    });
    e
}

/// The models the API at `base` lists for `key`, bounded by [`CHECK_TIME`] on a worker thread, so
/// a server that trickles its answer can't hold the caller longer.
pub(crate) fn list_models(kind: BackendKind, label: &str, base: Option<&str>, key: Option<String>) -> Result<Vec<String>, ToolError> {
    let (label, base) = (label.to_string(), base.map(str::to_string));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("ai-key-check".into())
        .spawn(move || {
            let _ = tx.send(list_now(kind, &label, base.as_deref(), key));
        })
        .map_err(|e| ToolError::internal(e.to_string()))?;
    rx.recv_timeout(CHECK_TIME).map_err(|_| {
        ToolError::new(ErrorCode::TimedOut, format!("The server didn't list its models within {} s.", CHECK_TIME.as_secs()))
            .with_remediation("Check the address and your connection, then try again.")
    })?
}

fn list_now(kind: BackendKind, label: &str, base: Option<&str>, key: Option<String>) -> Result<Vec<String>, ToolError> {
    let t = CHECK_TIME;
    match kind {
        BackendKind::Anthropic => anthropic::Anthropic::new(base, key.unwrap_or_default(), "").list_models(t),
        BackendKind::Openai => openai::OpenAi::official(base, key.unwrap_or_default(), "").list_models(t),
        BackendKind::Openrouter => openai::OpenAi::openrouter(base, key.unwrap_or_default(), "").list_models(t),
        BackendKind::Gemini => openai::OpenAi::gemini(base, key.unwrap_or_default(), "").list_models(t),
        BackendKind::OpenaiCompatible => {
            let b = base.unwrap_or("http://127.0.0.1:11434/v1");
            // OpenCode Go lists its models at /v1, not under its chat path.
            let probe = if openai::is_opencode_go(b) { crate::opencode_go_models_base(b) } else { b.to_string() };
            openai::OpenAi::compatible(label, &probe, key, "").list_models(t)
        }
        BackendKind::AgentCli => Err(ToolError::invalid("An agent app signs in by itself; it doesn't take an API key.")),
    }
}

/// A failed key check, said in terms of the key: a 401 or 403 is a rejected key, with where to get
/// a new one; anything else says nothing was saved and how to save without the check.
fn refused(e: ToolError, who: &str, key_url: Option<&str>, key: Option<&str>) -> ToolError {
    let status = e.details.as_ref().and_then(|d| d.get("status")).and_then(Value::as_u64);
    let out = match status {
        Some(401 | 403) => {
            let fix = match key_url {
                Some(url) => format!("Nothing was saved. Check that the whole key was copied, or make a new one at {url}."),
                None => "Nothing was saved. Check that the whole key was copied, or make a new one in the provider's dashboard.".to_string(),
            };
            ToolError::new(ErrorCode::NoPermission, format!("{who} rejected the key."))
                .with_remediation(fix)
                .with_details(json!({"status": status, "provider_message": e.message}))
        }
        Some(_) => {
            let fix = e.remediation.clone().unwrap_or_default();
            e.with_remediation(format!("Nothing was saved. {fix}"))
        }
        None => {
            let message = format!("Couldn't check the key with {who}: {}", e.message);
            ToolError::new(e.code, message).with_remediation(
                "Nothing was saved. Check your connection and the address, then try again. To save it without checking (offline set-up), \
                 turn the check off (`check: false`, or `--no-check` in the CLI).",
            )
        }
    };
    match key {
        Some(k) => scrub(out, k),
        None => out,
    }
}

/// The provider offer whose own address a backend of `kind` at `base` uses, if any.
pub fn own_offer(kind: BackendKind, base: Option<&str>) -> Option<ProviderOffer> {
    let base = base.map(|b| b.trim().trim_end_matches('/')).filter(|b| !b.is_empty());
    offers::offers().into_iter().find(|o| o.kind == kind && o.base_url.as_deref().map(|b| b.trim_end_matches('/')) == base)
}

/// An environment variable's value is sent with the key check and stored, so not every variable
/// may be read: a provider's own (from its offer) only goes to that provider at its own address
/// (`own`), and otherwise only a name ending in `_API_KEY` that isn't a provider's. Anything else
/// (`GITHUB_TOKEN`) could be another secret. Checked before anything is read or sent.
fn env_allowed(var: &str, own: Option<&ProviderOffer>) -> Result<(), ToolError> {
    let upper = var.to_ascii_uppercase();
    let is = |v: &String| v.eq_ignore_ascii_case(&upper);
    if own.is_some_and(|o| o.key_env.iter().any(is)) {
        return Ok(());
    }
    let fix = "Put the key in a variable named like MY_SERVER_API_KEY, or give it as `api_key` (`--key-stdin` in the CLI).";
    if let Some(owner) = offers::offers().into_iter().find(|o| o.key_env.iter().any(is)) {
        return Err(
            ToolError::invalid(format!("{var} holds {}'s key, so it's only sent to {} at its own address.", owner.name, owner.name)).with_remediation(fix)
        );
    }
    if !upper.ends_with("_API_KEY") {
        return Err(
            ToolError::invalid(format!("Only a variable whose name ends in _API_KEY is read for a key; {var} may hold another secret.")).with_remediation(fix)
        );
    }
    Ok(())
}

/// A key read from the environment variable `var`, which must be one [`env_allowed`] lets go to
/// where the key is sent (`own`: the provider at its own address, if that's where). Used by the
/// tools' `api_key_env` and the CLI's `--key-env` alike.
pub fn key_from_env(var: &str, own: Option<&ProviderOffer>) -> Result<String, ToolError> {
    let var = var.trim();
    if var.is_empty() || var.len() > 128 || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ToolError::invalid("The key's variable is the name of an environment variable, like ANTHROPIC_API_KEY."));
    }
    env_allowed(var, own)?;
    std::env::var(var).ok().filter(|k| !k.trim().is_empty()).ok_or_else(|| {
        ToolError::invalid(format!("The environment variable {var} isn't set where Craft Anything runs."))
            .with_remediation("Set it before starting the app or the MCP server, or give the key another way.")
    })
}

fn trimmed_key(key: Option<String>) -> Result<Option<String>, ToolError> {
    match key {
        Some(k) => {
            let k = k.trim().to_string();
            check_key(&k)?;
            Ok(Some(k))
        }
        None => Ok(None),
    }
}

fn unknown_offer(id: &str) -> ToolError {
    let shown: String = id.trim().chars().take(40).collect();
    let ids: Vec<String> = offers::offers().into_iter().map(|o| o.id).collect();
    ToolError::invalid(format!("There's no provider called {shown:?}.")).with_remediation(format!("Use one of: {}.", ids.join(", ")))
}

/// Check an AI from a provider offer before it's saved. Everything given is checked first; then,
/// with `check`, the provider lists its models for the key (bounded by [`CHECK_TIME`]), and a
/// refusal or no answer means nothing is to be saved. Without `check` nothing is sent anywhere.
/// The model is the one given, else the offer's default when listed (or not checked), else the
/// first listed model that can chat. `base_url` is another address for the same API (a proxy).
pub fn prepare_offer(
    offer_id: &str,
    name: Option<&str>,
    key: Option<String>,
    model: Option<&str>,
    base_url: Option<&str>,
    check: bool,
) -> Result<Prepared, ToolError> {
    let offer: ProviderOffer = offers::offer(offer_id).ok_or_else(|| unknown_offer(offer_id))?;
    let name = check_name(name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&offer.default_name))?;
    let key = trimmed_key(key)?;
    let model = check_model(model)?;
    let override_base = base_url.map(str::trim).filter(|b| !b.is_empty()).map(check_address).transpose()?;
    let own_address = override_base.is_none();
    let base = override_base.or_else(|| offer.base_url.clone());
    let mut listed = Vec::new();
    if check {
        // A key from the provider's own variable checks it when none is given (it isn't stored),
        // but only on the provider's own address: never sent to another.
        let env_key = || offer.key_env.iter().filter_map(|v| std::env::var(v).ok()).map(|k| k.trim().to_string()).find(|k| !k.is_empty());
        let probe_key = key.clone().or_else(|| if own_address { env_key() } else { None });
        if probe_key.is_none() && offer.needs_key {
            let fix = match &offer.key_url {
                Some(url) => format!("Make one at {url} and give it, or add it without the check and set the key later."),
                None => "Give its key, or add it without the check and set the key later.".to_string(),
            };
            return Err(ToolError::invalid(format!("{} needs an API key.", offer.name)).with_remediation(fix));
        }
        listed = list_models(offer.kind, &name, base.as_deref(), probe_key.clone())
            .map_err(|e| refused(e, &offer.name, offer.key_url.as_deref(), probe_key.as_deref()))?;
    }
    let model = match model {
        Some(m) => m,
        None if listed.is_empty() || listed.contains(&offer.default_model) => offer.default_model.clone(),
        None => detect::chat_model(&listed).unwrap_or_else(|| offer.default_model.clone()),
    };
    Ok(Prepared {
        entry: BackendEntry {
            config: BackendConfig { name, kind: offer.kind, model, base_url: base, ssh_host: None, enabled: true },
            options: BackendOptions::default(),
        },
        key,
        offer: Some(offer.id),
        checked: check,
        models_listed: listed.len(),
    })
}

/// Check an OpenAI-compatible server (or Anthropic's API) by its address before it's saved. With
/// `check` it's kept only when it answers with its models, and without a model the first one it
/// lists that can chat is used; without `check` nothing is sent and a model must be named.
pub fn prepare_server(name: &str, address: &str, model: Option<&str>, key: Option<String>, check: bool) -> Result<Prepared, ToolError> {
    let name = check_name(name)?;
    let address = check_address(address)?;
    let key = trimmed_key(key)?;
    let model = check_model(model)?;
    let kind = if anthropic::is_anthropic_api(&address) { BackendKind::Anthropic } else { BackendKind::OpenaiCompatible };
    let mut listed = Vec::new();
    let model = if check {
        listed = list_models(kind, &name, Some(&address), key.clone()).map_err(|e| {
            let why = e.message.clone();
            let e = e.with_remediation(format!(
                "Check the address (it usually ends in /v1), that the server is running and can be reached from this computer, and its key if it needs one. ({why})"
            ));
            match &key {
                Some(k) => scrub(e, k),
                None => e,
            }
        })?;
        match model {
            Some(m) => m,
            None => detect::chat_model(&listed).ok_or_else(|| {
                let reason = if listed.is_empty() { "has no models yet" } else { "doesn't list a model that can chat" };
                ToolError::new(ErrorCode::NotFound, format!("{address} {reason}."))
                    .with_remediation("Load a chat model on the server first, or enter the name of a chat model you know is available.")
            })?,
        }
    } else {
        model.ok_or_else(|| ToolError::invalid("Name the model: without the check the server isn't asked for its list."))?
    };
    Ok(Prepared {
        entry: BackendEntry {
            config: BackendConfig { name, kind, model, base_url: Some(address), ssh_host: None, enabled: true },
            options: BackendOptions::default(),
        },
        key,
        offer: None,
        checked: check,
        models_listed: listed.len(),
    })
}

/// Store the prepared key under the backend's name, put the backend in `cfg` and `save` it. A name
/// that's already set up is refused unless `replace`. Keys are stored by name, so a backend added
/// without a key never inherits one stored under its name (an old backend's, or one left behind by
/// a removal): it would be sent to the new address. A save that fails puts the key back as it was,
/// so nothing is left half-added.
pub fn save_added(
    prepared: Prepared,
    mut cfg: AiConfig,
    keys: &KeyStore,
    replace: bool,
    save: impl FnOnce(&AiConfig) -> Result<(), ToolError>,
) -> Result<AddedBackend, ToolError> {
    let Prepared { entry, key, offer, checked, models_listed } = prepared;
    let name = entry.config.name.clone();
    if !replace && cfg.get(&name).is_some() {
        let shown: String = name.chars().take(100).collect();
        return Err(ToolError::invalid(format!("An AI named {shown:?} is already set up."))
            .with_remediation("Give the new one another name, or replace it (`replace: true`, or `--replace` in the CLI); its stored key is then dropped unless a new one is given."));
    }
    // OpenCode Go's session id is made here, where saving the settings is already the job.
    if cfg.install_id.as_deref().is_none_or(|s| s.trim().is_empty()) && openai::is_opencode_go(entry.config.base_url.as_deref().unwrap_or("")) {
        cfg.install_id = Some(crate::new_session_id());
    }
    let before = keys.get(&name);
    let stored_in = match &key {
        Some(k) => Some(keys.set(&name, k)?),
        None if before.is_some() => {
            keys.delete(&name)?;
            None
        }
        None => None,
    };
    let added = AddedBackend {
        name: name.clone(),
        offer,
        kind: entry.config.kind,
        model: entry.config.model.clone(),
        base_url: entry.config.base_url.clone(),
        key_stored: stored_in.is_some(),
        key_stored_in: stored_in.map(str::to_string),
        checked,
        models_listed,
    };
    cfg.upsert(entry);
    if let Err(e) = save(&cfg) {
        match before {
            Some(old) => {
                let _ = keys.set(&name, &old);
            }
            None if key.is_some() => {
                let _ = keys.delete(&name);
            }
            None => {}
        }
        return Err(e);
    }
    Ok(added)
}

/// The key page of the provider a saved backend uses, if we know it.
fn key_url_of(entry: &BackendEntry) -> Option<String> {
    let base = entry.config.base_url.as_deref().map(|b| b.trim().trim_end_matches('/'));
    offers::offers()
        .into_iter()
        .find(|o| o.kind == entry.config.kind && (entry.config.kind != BackendKind::OpenaiCompatible || o.base_url.as_deref() == base))
        .and_then(|o| o.key_url)
}

/// Check a new key for a saved backend: the same rules as adding one, and with `check` its
/// provider must list its models for it. Returns the key to store (trimmed).
pub fn prepare_key(entry: &BackendEntry, key: &str, check: bool) -> Result<String, ToolError> {
    let key = key.trim();
    check_key(key)?;
    let c = &entry.config;
    if c.kind == BackendKind::AgentCli {
        return Err(ToolError::invalid(format!("{} is an agent app: it signs in by itself, not with an API key.", c.name)));
    }
    if check {
        if c.ssh_host.as_deref().is_some_and(|h| !h.trim().is_empty()) {
            return Err(ToolError::unavailable("A key for a server reached over SSH can't be checked from here.")
                .with_remediation("Save it without the check (`check: false`, or `--no-check` in the CLI)."));
        }
        let base = c.base_url.as_deref().filter(|b| !b.trim().is_empty());
        list_models(c.kind, &c.name, base, Some(key.to_string())).map_err(|e| refused(e, &c.name, key_url_of(entry).as_deref(), Some(key)))?;
    }
    Ok(key.to_string())
}

/// A key is stored or in the environment, or the backend needs none.
pub(crate) fn key_ready(b: &BackendEntry, keys: &KeyStore) -> bool {
    let name = &b.config.name;
    let base = b.config.base_url.as_deref().unwrap_or("");
    if b.config.kind == BackendKind::OpenaiCompatible {
        // A local server holds no key; a hosted one is only ready once its key is somewhere.
        if openai::is_opencode_go(base) || providers::of_base(base).is_some() {
            return keys.resolve(name, b.config.kind).is_some() || crate::go_env_key(base).is_some() || providers::env_key(base).is_some();
        }
        return true;
    }
    !b.config.kind.needs_key() || keys.resolve(name, b.config.kind).is_some()
}

/// The key store a tool uses: the one the app installed ([`crate::AiHub::install_key_store`]),
/// else the system's for a session that uses the person's own config folder, else a key file in
/// the session's own config folder (a test or isolated session never reaches the keychain).
pub(crate) fn key_store(session: &Session) -> KeyStore {
    if let Some(k) = session.ext.get::<KeyStore>() {
        return k.as_ref().clone();
    }
    let dir = session.config_dir();
    if dir == craft_anything_engine::settings::config_dir() { KeyStore::system() } else { KeyStore::file_only(dir.join("ai-keys.toml")) }
}

/// Save `cfg` as the session's `[ai]` settings: the settings file first, then the session, so a
/// save that fails changes nothing. For tools, which hold the session already.
pub(crate) fn save_in_session(session: &mut Session, cfg: &AiConfig) -> Result<(), ToolError> {
    let mut next = session.settings.clone();
    next.ai = cfg.to_table()?;
    next.save_to(&session.config_dir().join("config.toml"))?;
    session.settings = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crash cases: a key that is too long, has a line break or isn't ASCII is refused before
    /// anything is read, stored or sent (the address here would refuse any connection).
    #[test]
    fn a_key_that_isnt_one_is_refused_before_any_io() {
        let long = "k".repeat(600);
        for bad in [long.as_str(), "sk-te\nst", "sk-tëst", "sk test", "sk-\u{0}"] {
            let e = prepare_offer("anthropic", None, Some(bad.into()), None, Some("http://127.0.0.1:0"), true).unwrap_err();
            assert_eq!(e.code, ErrorCode::BadInput, "{bad:?}");
            assert!(!format!("{e:?}").contains(bad), "the key is never in the error");
            assert_eq!(check_key(bad).unwrap_err().code, ErrorCode::BadInput);
        }
        assert!(check_key(&"k".repeat(512)).is_ok());
        assert!(prepare_offer("anthropic", None, Some(" ".into()), None, None, false).is_err(), "an empty key");
        assert!(prepare_offer("nope", None, None, None, None, false).unwrap_err().remediation.unwrap().contains("anthropic"));
        assert!(prepare_offer("anthropic", Some("two\nlines"), None, None, None, false).is_err());
        assert!(prepare_offer("anthropic", None, None, None, Some("ftp://x"), false).is_err());
    }

    #[test]
    fn scrubbing_takes_the_key_out_everywhere() {
        let e = ToolError::invalid("bad sk-test-1 here").with_remediation("not sk-test-1").with_details(json!({"a": ["sk-test-1"]}));
        let e = scrub(e, "sk-test-1");
        assert!(!format!("{e:?}").contains("sk-test-1"));
        assert_eq!(e.details.unwrap()["a"][0], "[key hidden]");
    }

    /// Without the check nothing is sent: the offer's model and address are used as they are.
    #[test]
    fn without_the_check_nothing_is_sent() {
        let p = prepare_offer("anthropic", None, Some("sk-test".into()), None, Some("http://127.0.0.1:0"), false).unwrap();
        assert_eq!((p.entry.config.name.as_str(), p.entry.config.model.as_str(), p.checked), ("Claude", anthropic::DEFAULT_MODEL, false));
        assert_eq!(p.entry.config.base_url.as_deref(), Some("http://127.0.0.1:0"));
        let p = prepare_offer("groq", Some("Mine"), None, Some("m"), None, false).unwrap();
        assert_eq!(
            (p.entry.config.kind, p.entry.config.base_url.as_deref(), p.key),
            (BackendKind::OpenaiCompatible, Some("https://api.groq.com/openai/v1"), None)
        );
        assert!(prepare_server("Box", "http://127.0.0.1:0/v1", None, None, false).is_err(), "a model is needed");
        assert_eq!(prepare_server("Box", "https://api.anthropic.com", Some("m"), None, false).unwrap().entry.config.kind, BackendKind::Anthropic);
    }

    /// Keys are stored by name: a name already set up is refused unless replaced, a backend added
    /// without a key never inherits the one stored under its name, and a failed save puts the
    /// stored key back as it was.
    #[test]
    fn an_added_backend_never_inherits_a_stored_key() {
        let d = tempfile::tempdir().unwrap();
        let keys = KeyStore::file_only(d.path().join("keys.toml"));
        let evil = || prepare_server("Claude", "http://127.0.0.1:0/v1", Some("m"), None, false).unwrap();
        let mut cfg = AiConfig::default();
        cfg.upsert(prepare_offer("anthropic", None, None, None, None, false).unwrap().entry);
        keys.set("Claude", "sk-test-old").unwrap();
        let ok = |_: &AiConfig| Ok(());
        let e = save_added(evil(), cfg.clone(), &keys, false, ok).unwrap_err();
        assert_eq!(e.code, ErrorCode::BadInput);
        assert!(e.remediation.unwrap().contains("replace"));
        assert_eq!(keys.get("Claude").as_deref(), Some("sk-test-old"), "refused: nothing changed");

        let failing = |_: &AiConfig| Err(ToolError::internal("disk full"));
        assert!(save_added(evil(), cfg.clone(), &keys, true, failing).is_err());
        assert_eq!(keys.get("Claude").as_deref(), Some("sk-test-old"), "a failed replace puts the key back");
        let mut new_key = evil();
        new_key.key = Some("sk-test-new".into());
        assert!(save_added(new_key.clone(), cfg.clone(), &keys, true, failing).is_err());
        assert_eq!(keys.get("Claude").as_deref(), Some("sk-test-old"), "a failed replace with a key puts the old one back");

        let added = save_added(evil(), cfg.clone(), &keys, true, ok).unwrap();
        assert!(!added.key_stored);
        assert_eq!(keys.get("Claude"), None, "a replace without a key drops the old one");
        keys.set("Claude", "sk-test-old").unwrap();
        assert!(save_added(new_key, cfg, &keys, true, ok).unwrap().key_stored);
        assert_eq!(keys.get("Claude").as_deref(), Some("sk-test-new"));

        // A key left behind under a name nobody uses any more isn't picked up by a new backend.
        keys.set("Ghost", "sk-test-left").unwrap();
        let ghost = prepare_server("Ghost", "http://127.0.0.1:0/v1", Some("m"), None, false).unwrap();
        save_added(ghost, AiConfig::default(), &keys, false, ok).unwrap();
        assert_eq!(keys.get("Ghost"), None);
    }

    /// A session with its own config folder and no store installed never reaches the keychain.
    #[test]
    fn an_isolated_session_keeps_its_keys_in_its_own_folder() {
        let session = craft_anything_engine::testing::session();
        let store = format!("{:?}", key_store(&session));
        assert!(store.contains("use_keychain: false") && store.contains(&format!("{:?}", session.config_dir().join("ai-keys.toml"))), "{store}");
    }

    /// A connection refused (port 0) saves nothing and says how to save without the check.
    #[test]
    fn no_answer_is_refused_unless_unchecked() {
        let e = prepare_offer("anthropic", None, Some("sk-test-refused".into()), None, Some("http://127.0.0.1:0"), true).unwrap_err();
        assert!(e.remediation.as_deref().unwrap_or_default().contains("check: false"), "{e:?}");
        assert!(!format!("{e:?}").contains("sk-test-refused"));
    }
}
