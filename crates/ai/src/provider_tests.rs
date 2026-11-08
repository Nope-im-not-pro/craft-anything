//! Adding an AI from a provider offer, and the AI set-up tools, against stand-in servers. No real
//! network, keychain or settings: keys go to a temp key file, settings to temp folders.

use super::*;
use crate::tests::mock;
use craft_anything_engine::{CallOptions, Caller, ToolOutput};
use serde_json::{Value, json};

const JSON: &str = "application/json";
const CLAUDE_MODELS: &str = r#"{"data":[{"type":"model","id":"claude-sonnet-5-5"},{"type":"model","id":"claude-opus-5-5"}],"has_more":false}"#;
const REJECTED: &str = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key sk-test-rejected"}}"#;

struct Setup {
    hub: AiHub,
    dir: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::shared(craft_anything_engine::testing::session(), crate::tools());
        let hub = AiHub::with_stores(engine, dir.path().join("config.toml"), KeyStore::file_only(dir.path().join("ai-keys.toml")));
        Setup { hub, dir }
    }

    fn keys(&self) -> KeyStore {
        KeyStore::file_only(self.dir.path().join("ai-keys.toml"))
    }

    fn files(&self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        (std::fs::read(self.dir.path().join("config.toml")).ok(), std::fs::read(self.dir.path().join("ai-keys.toml")).ok())
    }

    /// Call a tool on a thread with a time limit: a tool that locked its own session would hang.
    fn call(&self, id: &'static str, params: Value) -> Result<Value, ToolError> {
        let engine = self.hub.engine.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(engine.call(id, params, CallOptions::new(Caller::Cli)));
        });
        match rx.recv_timeout(Duration::from_secs(60)).expect("the tool didn't finish in time") {
            Ok(ToolOutput::Value(v)) => Ok(v),
            Ok(other) => panic!("{other:?}"),
            Err(e) => Err(e),
        }
    }

    /// The settings file the tools save to (the session's own config folder).
    fn tool_config(&self) -> AiConfig {
        let path = self.hub.engine.session().config_dir().join("config.toml");
        AiConfig::from_settings(&Settings::load_from(&path))
    }
}

#[test]
fn the_claude_api_is_added_with_a_checked_key() {
    let s = Setup::new();
    let (url, seen) = mock(vec![(200, JSON, CLAUDE_MODELS.into())]);
    let added = s.hub.add_provider("anthropic", None, Some(" sk-test-ok ".into()), None, Some(&url), true).unwrap();
    assert_eq!((added.name.as_str(), added.kind, added.model.as_str()), ("Claude", BackendKind::Anthropic, "claude-opus-5-5"));
    assert!(added.key_stored && added.checked && added.models_listed == 2, "{added:?}");
    assert_eq!(added.key_stored_in.as_deref(), Some("file"));
    assert!(!serde_json::to_string(&added).unwrap().contains("sk-test-ok"));
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen[0].path, "/v1/models");
    assert_eq!(seen[0].header("x-api-key"), Some("sk-test-ok"));
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-ok"));
    let saved = AiConfig::from_settings(&Settings::load_from(&s.dir.path().join("config.toml")));
    let b = saved.get("Claude").unwrap();
    assert_eq!((b.config.kind, b.config.base_url.as_deref()), (BackendKind::Anthropic, Some(url.as_str())));
    assert!(s.hub.has_key("Claude"));
    // A model named is kept; the offer's default isn't listed, so the first that can chat is used.
    let (url, _) = mock(vec![(200, JSON, r#"{"data":[{"id":"claude-haiku-5-5"}]}"#.into())]);
    let added = s.hub.add_provider("anthropic", Some("Small"), Some("sk-test-2".into()), None, Some(&url), true).unwrap();
    assert_eq!(added.model, "claude-haiku-5-5");
}

#[test]
fn a_rejected_key_saves_nothing_and_is_never_shown() {
    let s = Setup::new();
    s.hub.add_provider("openai", None, Some("sk-test-other".into()), Some("gpt-5"), None, false).unwrap();
    let before = s.files();
    let (url, _) = mock(vec![(401, JSON, REJECTED.into())]);
    let e = s.hub.add_provider("anthropic", None, Some("sk-test-rejected".into()), None, Some(&url), true).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoPermission);
    assert!(e.remediation.as_deref().unwrap().contains("https://console.anthropic.com/settings/keys"), "{e:?}");
    assert!(!format!("{e:?}").contains("sk-test-rejected"), "{e:?}");
    assert_eq!(s.files(), before, "neither the settings nor the key file changed");
    assert!(s.hub.config().get("Claude").is_none());
}

#[test]
fn no_answer_saves_nothing_unless_the_check_is_off() {
    let s = Setup::new();
    let e = s.hub.add_provider("anthropic", None, Some("sk-test".into()), None, Some("http://127.0.0.1:0"), true).unwrap_err();
    assert!(e.remediation.unwrap().contains("--no-check"));
    assert_eq!(s.files(), (None, None));
    let added = s.hub.add_provider("anthropic", None, Some("sk-test".into()), None, Some("http://127.0.0.1:0"), false).unwrap();
    assert!(!added.checked && added.key_stored);
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test"));
    // Not a model list at all: refused too, and nothing saved.
    let (url, _) = mock(vec![(200, "text/html", "<html>hello</html>".into())]);
    assert!(s.hub.add_provider("openai", None, Some("sk-test".into()), None, Some(&url), true).is_err());
    assert!(s.hub.config().get("OpenAI").is_none());
}

/// Hostile model lists: wrong shapes give an empty list (the offer's model is used) or an error,
/// never a panic; a 9 MB body is read within its bounds.
#[test]
fn odd_model_lists_never_crash() {
    let s = Setup::new();
    let big = format!(r#"{{"data":[{{"id":"gpt-5","pad":"{}"}}]}}"#, "x".repeat(9 * 1024 * 1024));
    for (i, body) in [r#"{"data": 5}"#.to_string(), "[]".to_string(), r#"{"data":[{"id":7},null,"x"]}"#.to_string(), big].into_iter().enumerate() {
        let (url, _) = mock(vec![(200, JSON, body)]);
        let name = format!("Odd {i}");
        match s.hub.add_provider("openai", Some(&name), Some("sk-test".into()), None, Some(&url), true) {
            Ok(added) => assert_eq!(added.model, openai::OPENAI_DEFAULT_MODEL),
            Err(e) => assert_ne!(e.code, ErrorCode::Internal, "{e:?}"),
        }
    }
}

#[test]
fn a_saved_key_is_changed_only_when_the_new_one_works() {
    let s = Setup::new();
    s.hub.add_provider("anthropic", None, Some("sk-test-old".into()), None, Some("http://127.0.0.1:0"), false).unwrap();
    let (url, _) = mock(vec![(401, JSON, REJECTED.into())]);
    let mut cfg = s.hub.config();
    cfg.backends[0].config.base_url = Some(url.clone());
    s.hub.save_config(&cfg).unwrap();
    let e = s.hub.set_backend_key("Claude", "sk-test-rejected", true).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoPermission);
    assert!(!format!("{e:?}").contains("sk-test-rejected"));
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-old"));
    assert_eq!(s.hub.set_backend_key("Claude", "bad key", false).unwrap_err().code, ErrorCode::BadInput);
    let (url, seen) = mock(vec![(200, JSON, CLAUDE_MODELS.into())]);
    let mut cfg = s.hub.config();
    cfg.backends[0].config.base_url = Some(url);
    s.hub.save_config(&cfg).unwrap();
    assert_eq!(s.hub.set_backend_key("Claude", "sk-test-new", true).unwrap(), "file");
    assert_eq!(seen.lock().unwrap()[0].header("x-api-key"), Some("sk-test-new"));
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-new"));
    assert!(s.hub.set_backend_key("Nobody", "sk-test", false).is_err());
}

/// Every AI set-up tool runs through the engine without locking the session it was called with,
/// keeps keys out of its output and the journal, and saves to the session's settings.
#[test]
fn the_set_up_tools_work_headless() {
    let s = Setup::new();
    let providers = s.call("ai_providers_list", json!({})).unwrap();
    assert!(providers["providers"].as_array().unwrap().iter().any(|p| p["id"] == "anthropic" && p["needs_key"] == true));

    for bad in [json!({"offer": "anthropic", "api_key": "k".repeat(600)}), json!({"offer": "anthropic", "api_key": "sk-te\nst"})] {
        assert_eq!(s.call("ai_backend_add", bad).unwrap_err().code, ErrorCode::BadInput);
    }
    let both = json!({"offer": "anthropic", "address": "http://127.0.0.1:0/v1", "api_key": "sk-test"});
    assert_eq!(s.call("ai_backend_add", both).unwrap_err().code, ErrorCode::BadInput);
    let both_keys = json!({"offer": "anthropic", "api_key": "sk-test", "api_key_env": "CRAFT_ANYTHING_TEST_UNSET_KEY_VAR"});
    assert_eq!(s.call("ai_backend_add", both_keys).unwrap_err().code, ErrorCode::BadInput);
    let unset = json!({"offer": "anthropic", "api_key_env": "CRAFT_ANYTHING_TEST_UNSET_KEY_VAR"});
    assert!(s.call("ai_backend_add", unset).unwrap_err().message.contains("CRAFT_ANYTHING_TEST_UNSET_KEY_VAR"));

    let (url, _) = mock(vec![(401, JSON, REJECTED.into())]);
    let e = s.call("ai_backend_add", json!({"offer": "anthropic", "api_key": "sk-test-rejected", "base_url": url})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoPermission);
    assert!(s.tool_config().get("Claude").is_none() && s.keys().get("Claude").is_none());

    let (url, seen) = mock(vec![(200, JSON, CLAUDE_MODELS.into())]);
    let params = json!({"offer": "anthropic", "api_key": "sk-test-tool", "base_url": url});
    assert!(craft_anything_engine::journal::bounded(&params).to_string().contains("(redacted)"));
    assert!(!craft_anything_engine::journal::bounded(&params).to_string().contains("sk-test-tool"));
    let added = s.call("ai_backend_add", params).unwrap();
    assert_eq!((added["name"].as_str(), added["key_stored"].as_bool()), (Some("Claude"), Some(true)));
    assert!(!added.to_string().contains("sk-test-tool"));
    assert_eq!(seen.lock().unwrap()[0].header("x-api-key"), Some("sk-test-tool"));
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-tool"));
    assert!(s.tool_config().get("Claude").is_some(), "saved to the session's settings file");
    assert!(AiConfig::from_settings(&s.hub.engine.session().settings).get("Claude").is_some(), "and the session");

    let list = s.call("ai_backends_list", json!({})).unwrap();
    assert_eq!(list["backends"][0]["key_stored"], true);
    assert!(!list.to_string().contains("sk-test-tool"));

    let (url, _) = mock(vec![(401, JSON, REJECTED.into())]);
    let mut cfg = AiConfig::from_settings(&s.hub.engine.session().settings);
    cfg.backends[0].config.base_url = Some(url);
    s.hub.save_config(&cfg).unwrap();
    let e = s.call("ai_backend_key_set", json!({"name": "Claude", "api_key": "sk-test-rejected"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NoPermission);
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-tool"), "the old key stays");
    let set = s.call("ai_backend_key_set", json!({"name": "Claude", "api_key": "sk-test-later", "check": false})).unwrap();
    assert_eq!((set["key_stored"].as_bool(), set["checked"].as_bool()), (Some(true), Some(false)));
    assert_eq!(s.keys().get("Claude").as_deref(), Some("sk-test-later"));

    // A server by its address, tested with a tiny chat.
    let reply =
        "data: {\"choices\":[{\"delta\":{\"content\":\"ready\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    let (url, _) = mock(vec![(200, "text/event-stream", reply.into())]);
    let server = json!({"address": format!("{url}/v1"), "name": "Box", "model": "qwen3:8b", "check": false});
    assert_eq!(s.call("ai_backend_add", server).unwrap()["model"], "qwen3:8b");
    let report = s.call("ai_backend_test", json!({"name": "Box"})).unwrap();
    assert_eq!(report["reply"], "ready", "{report}");
    assert!(s.call("ai_backend_test", json!({"name": "Nobody"})).is_err());

    assert_eq!(s.call("ai_backend_remove", json!({"name": "Claude"})).unwrap()["removed"], true);
    assert!(s.keys().get("Claude").is_none() && s.tool_config().get("Claude").is_none());
    assert_eq!(s.call("ai_backend_remove", json!({"name": "Claude"})).unwrap()["removed"], false);
}

/// The AI layer's tools follow the id scheme (docs/tool-ids.md), and the set-up tools run in Clean
/// room: they touch no target.
#[test]
fn the_ai_tools_follow_the_id_scheme_and_run_in_clean_room() {
    for spec in crate::tools() {
        let mut words = spec.id.split('_');
        assert!(words.next().is_some_and(|a| craft_anything_engine::registry::AREAS.contains(&a)) && words.count() >= 1, "{}", spec.id);
    }
    for spec in backend_tools::specs() {
        assert!(spec.id.starts_with("ai_"), "{}", spec.id);
        assert!(craft_anything_engine::clean_room::allows(spec.id), "{} isn't allowed in Clean room", spec.id);
        assert!(!spec.effects.needs_consent(), "{}", spec.id);
    }
}
