//! End-to-end tests: a tiny std TcpListener HTTP server stands in for the model APIs, and a real
//! engine runs the tools. No real network.

use super::*;
use craft_anything_platform::sync::Mutex;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

/// #1096: an AI's timeout_secs sets how long its streamed answer may take; unset or 0 is an hour.
#[test]
fn an_ais_own_timeout_bounds_its_streamed_answer() {
    let with = |timeout_secs| BackendOptions { timeout_secs, ..Default::default() };
    assert_eq!(stream_limit(&with(None)), http::STREAM_BODY);
    assert_eq!(stream_limit(&with(Some(0))), http::STREAM_BODY);
    assert_eq!(stream_limit(&with(Some(4 * 3600))), std::time::Duration::from_secs(4 * 3600));
}

/// The model list lives at /v1 on the service root, not under the chat path's /openai/v1.
#[test]
fn opencode_go_lists_models_at_the_documented_url() {
    assert_eq!(format!("{}/models", opencode_go_models_base(openai::OPENCODE_GO_BASE)), "https://opencode.ai/inference/go/v1/models");
    assert_eq!(opencode_go_models_base("https://opencode.ai/inference/go/openai/v1/"), "https://opencode.ai/inference/go/v1");
    assert_eq!(opencode_go_models_base("http://127.0.0.1:11434/v1"), "http://127.0.0.1:11434/v1");
}

#[derive(Debug, Clone)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Seen {
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
}

/// Serve canned responses `(status, content_type, body)` in order; record requests.
fn mock(responses: Vec<(u16, &'static str, String)>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for (status, ctype, reply) in responses {
            let Ok((stream, _)) = l.accept() else { return };
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            let mut headers = Vec::new();
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        len = v.trim().parse().unwrap();
                    }
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let mut buf = vec![0; len];
            r.read_exact(&mut buf).unwrap();
            let body = serde_json::from_slice(&buf).unwrap_or(Value::Null);
            s2.lock().unwrap().push(Seen { path, headers, body });
            let mut w = stream;
            let reason = if status == 200 { "OK" } else { "Error" };
            write!(w, "HTTP/1.1 {status} {reason}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            w.flush().unwrap();
        }
    });
    (url, seen)
}

/// A stand-in server on a free port that answers every request (and any number of them, unlike
/// [`mock`]) with `status` and `body`; its `/v1` address.
pub(crate) fn serve(status: &'static str, body: &'static str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let (mut got, mut buf) = (Vec::new(), [0u8; 4096]);
            while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            if !got.is_empty() {
                let _ = write!(s, "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            }
        }
    });
    base
}

/// #48: a coding agent's test only runs `--version`, and its report says so.
#[test]
fn a_coding_agent_test_says_it_only_checked_the_version() {
    let (h, _d) = hub();
    let config = BackendConfig { name: "Agent".into(), kind: BackendKind::AgentCli, model: "opencode".into(), base_url: None, ssh_host: None, enabled: true };
    // cargo stands in for the agent's program: it's there wherever these tests run.
    let options = BackendOptions { command: Some("cargo".into()), ..Default::default() };
    h.add_backend_with(BackendEntry { config, options }, None).unwrap();
    let r = h.test_backend("Agent").unwrap();
    assert!(r.version_only && r.reply.starts_with("cargo "), "{r:?}");
    assert_eq!(serde_json::to_value(&r).unwrap()["version_only"], true, "the CLI's ai test prints it");
    assert_eq!((r.program.as_deref(), r.saved_program_replaced), (Some("cargo"), None), "the saved program ran");
}

/// #721: a saved path an update removed. A test that passes through the usual program says so, and
/// doctor doesn't blame PATH when nothing can be found.
#[test]
fn a_saved_program_that_is_gone_is_named_by_test_and_doctor() {
    let (h, _d) = hub();
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("old-build").join("agent").display().to_string();
    let backend = |name: &str, model: &str| BackendEntry {
        config: BackendConfig { name: name.into(), kind: BackendKind::AgentCli, model: model.into(), base_url: None, ssh_host: None, enabled: true },
        options: BackendOptions { command: Some(gone.clone()), ..Default::default() },
    };
    // cargo stands in for the agent's usual program: it's there wherever these tests run.
    h.add_backend_with(backend("Usual", "cargo"), None).unwrap();
    h.add_backend_with(backend("Nowhere", "no-such-agent-program"), None).unwrap();

    let r = h.test_backend("Usual").unwrap();
    assert_eq!((r.program.as_deref(), r.saved_program_replaced.as_deref()), (Some("cargo"), Some(gone.as_str())));

    let reach = h.check_reachability();
    let nowhere = reach.iter().find(|(name, _)| name == "Nowhere").map(|(_, r)| r.clone()).unwrap();
    assert_eq!(nowhere, Err(format!("there's no program at {gone}")));
}

/// Every known agent with an adapter of its own is offered when adding a coding agent.
#[test]
fn every_agent_with_its_own_adapter_is_offered() {
    let adapters = cli_agents::adapters::AgentAdapters::built_in();
    let own: Vec<&str> = cli_agents::KNOWN.iter().map(|(id, _, _)| *id).filter(|id| adapters.for_agent(id).ids().contains(id)).collect();
    assert!(own.contains(&"maki"), "{own:?}");
    for id in own {
        assert!(BackendKind::AgentCli.suggested_models().contains(&id), "{id} isn't offered");
    }
}

/// #37: doctor asks a local server for its models instead of trusting an open port.
#[test]
fn a_local_server_is_reachable_only_when_it_lists_models() {
    let (h, _d) = hub();
    add(&h, "Web page", BackendKind::OpenaiCompatible, &serve("404 Not Found", "404 page not found"), None);
    add(&h, "Real", BackendKind::OpenaiCompatible, &serve("200 OK", r#"{"data":[{"id":"qwen3-8b"}]}"#), None);
    let reach = h.check_reachability();
    let of = |n: &str| reach.iter().find(|(name, _)| name == n).map(|(_, r)| r.clone()).unwrap();
    assert!(of("Web page").is_err_and(|e| e.contains("404 page not found")), "{reach:?}");
    assert!(of("Real").is_ok(), "{reach:?}");
}

fn sse(events: &[Value]) -> String {
    events.iter().map(|e| format!("event: {}\r\ndata: {}\r\n\r\n", e["type"].as_str().unwrap_or("message"), e)).collect()
}

fn chat_sse(chunks: &[Value]) -> String {
    let mut s: String = chunks.iter().map(|c| format!("data: {c}\n\n")).collect();
    s.push_str("data: [DONE]\n\n");
    s
}

fn hub() -> (AiHub, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let h = AiHub::with_stores(engine, dir.path().join("config.toml"), KeyStore::file_only(dir.path().join("ai-keys.toml")));
    (h, dir)
}

/// A named backend stays off for chat, including before API or agent setup can run.
#[test]
fn disabled_backends_are_refused_when_chat_starts() {
    let (h, _dir) = hub();
    for kind in [BackendKind::OpenaiCompatible, BackendKind::AgentCli] {
        // A malformed tunnel port fails before opening SSH if API setup is reached.
        let (base_url, ssh_host) =
            if kind == BackendKind::OpenaiCompatible { ("http://127.0.0.1:not-a-port/v1", Some("unused".into())) } else { ("http://127.0.0.1:0/v1", None) };
        h.add_backend(
            BackendConfig { name: "Off".into(), kind, model: "no-such-agent".into(), base_url: Some(base_url.into()), ssh_host, enabled: false },
            None,
        )
        .unwrap();
        let error = h.start(Some("Off"), false).err().expect("chat must refuse a disabled backend");
        assert_eq!(error.code, ErrorCode::BadSettings);
        assert!(error.message.contains("Off") && error.message.contains("turned off"), "{error:?}");
        assert!(error.remediation.as_deref().is_some_and(|s| s.contains("Settings → AI")), "{error:?}");
    }
}

/// Compare checks either position; enabled named and automatic chat choices still work.
#[test]
fn disabled_backends_are_refused_on_either_side_of_compare() {
    let (h, _dir) = hub();
    for (name, enabled) in [("Off", false), ("On", true), ("Other", true)] {
        h.add_backend(
            BackendConfig {
                name: name.into(),
                kind: BackendKind::OpenaiCompatible,
                model: "test-model".into(),
                base_url: Some("http://127.0.0.1:0/v1".into()),
                ssh_host: None,
                enabled,
            },
            None,
        )
        .unwrap();
    }
    for (left, right) in [("Off", "On"), ("On", "Off")] {
        let error = h.compare(left, right, true).err().expect("compare must refuse a disabled backend");
        assert_eq!(error.code, ErrorCode::BadSettings);
        assert!(error.message.contains("Off") && error.message.contains("turned off"), "{error:?}");
    }
    assert!(h.start(Some("On"), false).is_ok());
    assert!(h.start(None, false).is_ok());
    assert!(h.compare("On", "Other", false).is_ok());
    let unknown = h.start(Some("Missing"), false).err().unwrap();
    assert!(unknown.message.contains("No AI backend named"), "{unknown:?}");
}

/// A CLI stand-in checks saved selections, and an explicit one runs natively with our MCP server.
#[cfg(unix)]
#[test]
fn opencode_selection_is_validated_saved_and_run_natively_with_our_mcp_server() {
    use std::os::unix::fs::PermissionsExt;
    let (h, dir) = hub();
    let program = dir.path().join("agent");
    let catalogue = dir.path().join("models.json");
    let seen = dir.path().join("arguments.txt");
    let quote = |p: &std::path::Path| ssh::shell_quote(&p.display().to_string());
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\napi) cat {};;\nrun) printf '%s\\n' \"$@\" > {}; cat \"$OPENCODE_CONFIG\" >> {}; printf 'done';;\n*) exit 8;;\nesac\n",
        quote(&catalogue),
        quote(&seen),
        quote(&seen)
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let model = json!({"id":"alias","providerID":"test","name":"Test","enabled":true,"type":"chat","variants":[{"id":"high"},{"id":"max"}]});
    std::fs::write(&catalogue, json!({"data":[model]}).to_string()).unwrap();
    let cfg = BackendConfig { name: "OpenCode".into(), kind: BackendKind::AgentCli, model: "opencode".into(), base_url: None, ssh_host: None, enabled: true };
    h.add_backend_with(
        BackendEntry { config: cfg, options: BackendOptions { command: Some(program.display().to_string()), acp: Some(true), ..Default::default() } },
        None,
    )
    .unwrap();
    assert_eq!(h.model_choices("OpenCode").unwrap(), ["test/alias"]);
    assert!(h.set_agent_model("OpenCode", Some("test/alias"), Some(Effort::Max)).unwrap());
    let saved = AiConfig::from_settings(&Settings::load_from(&dir.path().join("config.toml")));
    assert_eq!(saved.get("OpenCode").unwrap().options.cli_thinking, Some(Effort::Max));
    assert_eq!(saved.get("OpenCode").unwrap().config.model, "opencode");
    let mut conversation = h.start(Some("OpenCode"), false).unwrap();
    conversation.send("test prompt");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while conversation.is_busy() && Instant::now() < deadline {
        events.extend(conversation.poll());
        std::thread::sleep(Duration::from_millis(10));
    }
    events.extend(conversation.poll());
    events.retain(|event| !matches!(event, AgentEvent::Status { .. }));
    assert!(matches!(events.last(), Some(AgentEvent::TurnDone { .. })), "{events:?}");
    // The arguments, one per line, then the config file the run was handed.
    let ran = std::fs::read_to_string(&seen).unwrap();
    let args: Vec<&str> = ran.lines().collect();
    assert_eq!(args.get(..4), Some(["run", "--standalone", "--model", "test/alias#max"].as_slice()), "{ran}");
    let config: Value = serde_json::from_str(ran.lines().find(|l| l.starts_with("{\"mcp\"")).unwrap()).unwrap();
    assert_eq!(config["mcp"][cli_agents::MCP_NAME]["type"], "local", "the run gets our MCP server: {ran}");
    assert!(h.set_agent_model("OpenCode", Some("test/alias"), Some(Effort::Xhigh)).is_err());
    assert_eq!(h.options()["OpenCode"].cli_thinking, Some(Effort::Max), "a failed validation changes nothing");
    let stale = h.model_context("OpenCode").unwrap();
    h.set_enabled("OpenCode", false).unwrap();
    assert!(h.set_agent_selection(&stale, None, None).is_err(), "a late picker cannot overwrite newer settings");
    std::fs::write(&catalogue, r#"{"data":[]}"#).unwrap();
    assert!(h.set_model("OpenCode", "test/alias").is_err(), "removed models are not offered as a fallback");
    h.set_agent_model("OpenCode", None, None).unwrap();
    assert!(h.options()["OpenCode"].cli_model.is_none());
    assert!(h.options()["OpenCode"].cli_thinking.is_none());
}

/// Contention is an error, while an available session retains its actual optional project and remains saveable.
#[test]
fn model_context_distinguishes_a_busy_session_from_no_project() {
    use craft_anything_engine::project::{Project, Purpose};
    let (h, dir) = hub();
    let backend =
        BackendConfig { name: "OpenCode".into(), kind: BackendKind::AgentCli, model: "opencode".into(), base_url: None, ssh_host: None, enabled: true };
    h.add_backend(backend.clone(), None).unwrap();
    assert_eq!(h.model_context("OpenCode").unwrap().directory, None);
    let root = dir.path().join("test.cany");
    {
        let mut session = h.engine.session();
        session.open_project(Project::create(&root, "test", Purpose::Education).unwrap()).unwrap();
        assert!(h.model_context("OpenCode").is_err(), "lock contention must not capture a missing project");
    }
    let context = h.model_context("OpenCode").unwrap();
    assert_eq!(context.directory.as_ref(), Some(&root));
    assert_eq!(context.backend.config, backend);
    assert!(h.set_agent_selection(&context, None, None).unwrap(), "a capture after contention must remain saveable");
    assert!(h.model_context("missing").is_err());
}

#[test]
fn a_failed_save_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    // The settings "file" can't be written: its folder is a file.
    std::fs::write(dir.path().join("blocked"), "").unwrap();
    let engine = Arc::new(Engine::new(craft_anything_engine::testing::session()));
    let h = AiHub::with_stores(engine, dir.path().join("blocked").join("config.toml"), KeyStore::file_only(dir.path().join("ai-keys.toml")));
    let cfg = BackendConfig { name: "Kimi".into(), kind: BackendKind::Openrouter, model: "m".into(), base_url: None, ssh_host: None, enabled: true };
    assert!(h.add_backend(cfg, None).is_err());
    assert!(h.backends().is_empty(), "the app shows only what was saved");
}

/// #261: one bad price made the whole [ai] section read as empty, and adding a backend saved
/// that over the backends, roles and prices.
#[test]
fn ai_settings_that_cant_be_read_are_never_saved_over() {
    let (h, d) = hub();
    add(&h, "Claude Code", BackendKind::AgentCli, "http://127.0.0.1:9", None);
    add(&h, "Kimi K3.1 Pro (OpenRouter)", BackendKind::Openrouter, "http://127.0.0.1:9", None);
    h.set_role(Role::Builder, Some("Kimi K3.1 Pro (OpenRouter)")).unwrap();
    let file = d.path().join("config.toml");
    let mut bad = h.engine.session().settings.clone();
    let price: toml::Table = toml::from_str(
        "[m]
input_per_mtok = 1.0
",
    )
    .unwrap();
    bad.ai.insert("prices".into(), toml::Value::Table(price));
    bad.save_to(&file).unwrap();
    h.engine.session().settings = bad;
    let before = std::fs::read(&file).unwrap();
    let cfg = BackendConfig { name: "Other".into(), kind: BackendKind::Openrouter, model: "m".into(), base_url: None, ssh_host: None, enabled: true };
    let e = h.add_backend(cfg, Some("sk-test".into())).unwrap_err();
    assert_eq!(e.code, ErrorCode::BadSettings, "{e:?}");
    assert!(h.set_role(Role::Builder, None).is_err());
    assert!(h.set_enabled("Claude Code", false).is_err());
    assert!(h.remove_backend("Claude Code").is_err());
    let options = crate::model_options::SetModelOptions { backend: "Claude Code".into(), expected_cli_model: None, context: None };
    assert_eq!(h.set_model_options(&options).unwrap_err().code, ErrorCode::BadSettings);
    assert_eq!(std::fs::read(&file).unwrap(), before, "the settings file was changed");
    assert!(h.keys.get("Other").is_none(), "a refused add stored its key");
}

#[test]
fn the_person_chooses_who_builds() {
    let (h, _d) = hub();
    add(&h, "Claude Code", BackendKind::AgentCli, "http://127.0.0.1:9", None);
    add(&h, "Kimi K3.1 Pro (OpenRouter)", BackendKind::Openrouter, "http://127.0.0.1:9", Some("sk-test"));
    assert_eq!(h.chosen(Role::Builder), None, "automatic until chosen");
    assert!(h.set_role(Role::Builder, Some("Nobody")).is_err(), "only a configured backend");
    h.set_role(Role::Builder, Some("Kimi K3.1 Pro (OpenRouter)")).unwrap();
    assert_eq!(h.chosen(Role::Builder).as_deref(), Some("Kimi K3.1 Pro (OpenRouter)"));
    // The extractor is chosen the same way, and has no default.
    assert_eq!(h.chosen(Role::Extractor), None);
    h.set_role(Role::Extractor, Some("Kimi K3.1 Pro (OpenRouter)")).unwrap();
    assert_eq!(h.chosen(Role::Extractor).as_deref(), Some("Kimi K3.1 Pro (OpenRouter)"));
    // A removed backend can't stay the builder.
    h.remove_backend("Kimi K3.1 Pro (OpenRouter)").unwrap();
    assert_eq!((h.chosen(Role::Builder), h.chosen(Role::Extractor)), (None, None));
    h.set_role(Role::Builder, Some("Claude Code")).unwrap();
    h.set_role(Role::Builder, None).unwrap();
    assert_eq!(h.chosen(Role::Builder), None, "back to automatic");
}

/// #53: an agent app installed where it isn't looked for gets its program set (and cleared).
#[test]
fn an_agent_apps_program_is_set_and_cleared() {
    let (h, _d) = hub();
    add(&h, "OpenCode", BackendKind::AgentCli, "http://127.0.0.1:9", None);
    let program = |h: &AiHub| h.options().get("OpenCode").and_then(|o| o.command.clone());
    assert_eq!(program(&h), None);
    assert!(h.set_command("OpenCode", Some("  /opt/oc/bin/opencode ")).unwrap());
    assert_eq!(program(&h).as_deref(), Some("/opt/oc/bin/opencode"));
    assert!(!h.set_command("Nobody", Some("/x")).unwrap(), "only a configured backend");
    assert!(h.set_command("OpenCode", Some(" ")).unwrap());
    assert_eq!(program(&h), None, "blank finds it by its usual name again");
}

/// #467: a folder or app bundle can't be an agent's program (the file picker returns
/// `Antigravity.app` on a Mac); it's refused and the saved program stays as it was.
#[test]
fn an_app_bundle_is_refused_as_an_agents_program() {
    let (h, d) = hub();
    add(&h, "OpenCode", BackendKind::AgentCli, "http://127.0.0.1:9", None);
    let program = |h: &AiHub| h.options().get("OpenCode").and_then(|o| o.command.clone());
    let bundle = d.path().join("Antigravity.app");
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    let err = h.set_command("OpenCode", Some(&bundle.display().to_string())).unwrap_err();
    assert_eq!(err.code, ErrorCode::BadInput, "{err:?}");
    assert!(err.message.contains("is a folder"), "{}", err.message);
    assert_eq!(program(&h), None, "nothing is saved for a refused folder");
    // A file, and a bare name, are still accepted.
    let file = d.path().join("agy");
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    assert!(h.set_command("OpenCode", Some(&file.display().to_string())).unwrap());
    assert!(h.set_command("OpenCode", Some("agy")).unwrap());
    assert_eq!(program(&h).as_deref(), Some("agy"));
}

/// #423: a saved AI that is switched off isn't set up, so a copy found on the computer is offered
/// again (to be turned on); one that is on is left out.
#[test]
fn a_switched_off_agent_is_found_again() {
    let (h, _d) = hub();
    add(&h, "Claude Code", BackendKind::AgentCli, "http://127.0.0.1:9", None);
    let saved = h.backends().into_iter().next().unwrap();
    let found = |name: &str| Detected { suggestion: BackendConfig { name: name.into(), ..saved.clone() }, reason: "Found".into(), needs_key: false };
    let offered =
        |h: &AiHub| leave_out_set_up(vec![found("Claude Code"), found("Codex")], &h.backends()).into_iter().map(|d| d.suggestion.name).collect::<Vec<_>>();
    assert_eq!(offered(&h), ["Codex"], "on: Claude Code is set up, so it isn't offered");
    h.set_enabled("Claude Code", false).unwrap();
    assert_eq!(offered(&h), ["Claude Code", "Codex"], "off: it's found again, to be turned on");
    assert!(!is_on(&h.backends(), "Claude Code") && !is_on(&h.backends(), "Codex"));
}

/// Claude Code's folder (`CLAUDE_CONFIG_DIR`) is chosen per AI, so two can be two accounts; only
/// a full path, and only for an agent app that keeps one.
#[test]
fn claude_codes_folder_is_set_per_ai() {
    let (h, _d) = hub();
    let claude = |name: &str, ssh: Option<&str>| BackendConfig {
        name: name.into(),
        kind: BackendKind::AgentCli,
        model: "claude-code".into(),
        base_url: None,
        ssh_host: ssh.map(str::to_string),
        enabled: true,
    };
    h.add_backend(claude("Claude Code", None), None).unwrap();
    h.add_backend(claude("Claude Code 2", None), None).unwrap();
    h.add_backend(claude("Claude on box", Some("box")), None).unwrap();
    h.add_backend(BackendConfig { model: "codex".into(), ..claude("Codex", None) }, None).unwrap();
    let folder = |h: &AiHub, name: &str| h.options().get(name).and_then(|o| o.config_dir.clone());
    assert!(h.set_config_dir("Claude Code 2", Some(" ~/.claude-work ")).unwrap());
    assert_eq!((folder(&h, "Claude Code"), folder(&h, "Claude Code 2").as_deref()), (None, Some("~/.claude-work")));
    let here = if cfg!(windows) { r"C:\Claude\work" } else { "/Users/someone/Claude work" };
    assert!(h.set_config_dir("Claude Code", Some(here)).unwrap());
    assert_eq!(folder(&h, "Claude Code").as_deref(), Some(here));
    assert!(h.set_config_dir("Claude on box", Some("/home/someone/.claude-b")).unwrap(), "a path on that computer");
    assert!(h.set_config_dir("Claude Code", Some("claude-work")).unwrap_err().message.contains("isn't a full path"));
    assert!(h.set_config_dir("Codex", Some("/x")).unwrap_err().message.contains("doesn't keep a folder"));
    assert!(h.set_config_dir("Codex", None).unwrap(), "clearing is always fine");
    assert!(!h.set_config_dir("Nobody", Some("/x")).unwrap(), "only a configured backend");
    assert!(h.set_config_dir("Claude Code 2", Some(" ")).unwrap());
    assert_eq!(folder(&h, "Claude Code 2"), None, "blank is its usual folder again");
    assert_eq!(folder(&h, "Claude Code").as_deref(), Some(here), "each AI keeps its own");
    assert_eq!(crate::folder_var(&claude("x", None)), Some("CLAUDE_CONFIG_DIR"));
}

/// #123, #127: the model an AI uses is changed; Claude Code is offered Claude's models.
#[test]
fn an_ais_model_is_changed() {
    let (h, _d) = hub();
    let url = serve("200 OK", r#"{"object":"list","data":[{"id":"nomic-embed-text"},{"id":"qwen3:8b"}]}"#);
    h.add_server("Box", &url, Some("nomic-embed-text"), None).unwrap();
    assert_eq!(h.model_choices("Box").unwrap(), ["nomic-embed-text", "qwen3:8b"], "the server's own list");
    assert!(h.set_model("Box", " qwen3:8b ").unwrap());
    assert_eq!(h.backends().into_iter().find(|b| b.name == "Box").unwrap().model, "qwen3:8b");
    let claude =
        BackendConfig { name: "Claude Code".into(), kind: BackendKind::AgentCli, model: "claude-code".into(), base_url: None, ssh_host: None, enabled: true };
    h.add_backend(claude, None).unwrap();
    assert_eq!(h.model_choices("Claude Code").unwrap(), crate::anthropic::MODELS);
    assert!(h.set_model("Claude Code", "claude-sonnet-5-5").unwrap());
    let claude = h.options().get("Claude Code").and_then(|o| o.cli_model.clone());
    assert_eq!(claude.as_deref(), Some("claude-sonnet-5-5"), "the CLI is asked for it; the agent stays Claude Code");
    assert_eq!(h.backends().into_iter().find(|b| b.name == "Claude Code").unwrap().model, "claude-code");
    assert!(!h.set_model("Nobody", "m").unwrap());
    assert!(h.set_model("Box", " ").is_err());
}

/// Every Claude effort level can be chosen for the Claude API and Claude Code, and only for them.
#[test]
fn a_claude_ais_effort_is_changed() {
    let (h, _d) = hub();
    let b = |name: &str, kind, model: &str| BackendConfig { name: name.into(), kind, model: model.into(), base_url: None, ssh_host: None, enabled: true };
    h.add_backend(b("Claude Code", BackendKind::AgentCli, "claude-code"), None).unwrap();
    h.add_backend(b("Codex", BackendKind::AgentCli, "codex"), None).unwrap();
    let effort = |h: &AiHub, name: &str| {
        let config = h.backends().into_iter().find(|b| b.name == name).unwrap();
        crate::effort_in_use(&config, h.options().get(name))
    };
    assert_eq!(effort(&h, "Claude Code"), Some(Effort::High), "high until it's chosen");
    for level in Effort::ALL {
        assert!(h.set_effort("Claude Code", Some(level)).unwrap());
        assert_eq!(effort(&h, "Claude Code"), Some(level));
    }
    assert!(h.set_effort("Claude Code", None).unwrap());
    assert_eq!(effort(&h, "Claude Code"), Some(Effort::High), "back to the usual level");
    assert_eq!(effort(&h, "Codex"), None, "an agent that doesn't take Claude's levels");
    assert!(h.set_effort("Codex", Some(Effort::Max)).is_err());
    assert!(!h.set_effort("Nobody", Some(Effort::Low)).unwrap());
}

#[test]
fn copilot_uses_shared_selection_and_rejects_failed_discovery_without_saving() {
    let (hub, _directory) = hub();
    let config =
        BackendConfig { name: "GitHub Copilot".into(), kind: BackendKind::AgentCli, model: "copilot".into(), base_url: None, ssh_host: None, enabled: true };
    assert!(!config.kind.needs_key());
    assert_eq!(model_in_use(&config, None).as_deref(), Some("GitHub Copilot default"));
    hub.add_backend_with(
        BackendEntry {
            config,
            options: BackendOptions {
                cli_model: Some("chosen-copilot-model".into()),
                command: Some("craft-anything-no-such-test-agent".into()),
                ..Default::default()
            },
        },
        None,
    )
    .unwrap();
    assert!(hub.set_model("GitHub Copilot", "unavailable-model").is_err());
    let backend = hub.backends().into_iter().find(|backend| backend.name == "GitHub Copilot").unwrap();
    let options = hub.options();
    assert_eq!(backend.model, "copilot");
    assert_eq!(model_in_use(&backend, options.get(&backend.name)).as_deref(), Some("chosen-copilot-model"));
}

#[test]
fn copilot_context_is_saved_without_changing_shared_thinking() {
    use crate::config::CliContext;
    let (hub, _directory) = hub();
    let config = BackendConfig { name: "Copilot".into(), kind: BackendKind::AgentCli, model: "copilot".into(), base_url: None, ssh_host: None, enabled: true };
    hub.add_backend_with(
        BackendEntry {
            config: config.clone(),
            options: BackendOptions { cli_model: Some("test-model".into()), cli_thinking: Some(Effort::Max), ..Default::default() },
        },
        None,
    )
    .unwrap();
    let params =
        crate::model_options::SetModelOptions { backend: "Copilot".into(), expected_cli_model: Some("test-model".into()), context: Some(CliContext::Long) };
    hub.set_model_options(&params).unwrap();
    assert_eq!(hub.options()["Copilot"].cli_context, Some(CliContext::Long));
    assert_eq!(hub.options()["Copilot"].cli_thinking, Some(Effort::Max));
    hub.add_backend_with(BackendEntry { config, options: BackendOptions { cli_model: Some("another-model".into()), ..Default::default() } }, None).unwrap();
    assert_eq!(hub.options()["Copilot"].cli_context, None);
    assert_eq!(hub.options()["Copilot"].cli_reasoning_effort, None);
    assert!(hub.set_model_options(&params).is_err());
}

/// Exercise the installed CLI, not just our argument builder: the loopback model must see
/// exactly the allowed tools in every access mode, including genuinely tool-free questions.
/// Kept opt-in because it needs external programs, never because it needs a real account.
#[test]
#[ignore = "requires Copilot CLI and Node.js; uses an isolated CLI home and a loopback model, never a real account"]
fn copilot_cli_enforces_mcp_only_and_empty_tool_filters() {
    use crate::cli_agents::{Access, AcpConversation, AgentAdapter, ArgsCx, McpCommand};
    use craft_anything_platform::Process;

    /// Only the child's environment changes: no saved CLI credentials, inherited provider key
    /// or token helper may reach the fixture. The production adapter still supplies its policy.
    struct OfflineCopilot {
        url: String,
        home: PathBuf,
    }

    impl AgentAdapter for OfflineCopilot {
        fn ids(&self) -> &'static [&'static str] {
            &["copilot"]
        }

        fn args(&self, _context: &ArgsCx) -> Vec<String> {
            Vec::new()
        }

        fn process(&self, program: &str) -> Process {
            crate::cli_agents::adapters::Copilot
                .process(program)
                .env("HOME", &self.home)
                .env("COPILOT_HOME", &self.home)
                .env("COPILOT_OFFLINE", "true")
                .env("COPILOT_PROVIDER_BASE_URL", &self.url)
                .env("COPILOT_PROVIDER_TYPE", "openai")
                .env("COPILOT_MODEL", "craft-test")
                .env_remove("COPILOT_PROVIDER_API_KEY")
                .env_remove("COPILOT_PROVIDER_API_KEY_COMMAND")
                .env_remove("COPILOT_PROVIDERS_CONFIG")
                .env_remove("COPILOT_GITHUB_TOKEN")
                .env_remove("GH_TOKEN")
                .env_remove("GITHUB_TOKEN")
        }
    }

    for (access, mut expected) in [
        (Access::Analyse, vec!["craft-anything-core_target_triage"]),
        (Access::Build, vec!["craft-anything-core_target_triage"]),
        (Access::CleanRoomBuild, vec!["craft-anything-core_target_triage"]),
        (Access::Question, vec!["view", "grep", "glob"]),
        (Access::CleanRoomQuestion, Vec::new()),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("mcp.cjs");
        // Offer one registered tool and one that must be filtered out. Record requests so a
        // question that accidentally starts the server is caught even if it calls no tool.
        std::fs::write(
            &script,
            r#"const readline = require('node:readline');
const input = readline.createInterface({input: process.stdin});
input.on('line', line => {
    const request = JSON.parse(line);
    require('node:fs').appendFileSync(__dirname + '/mcp-requests.log', request.method + '\n');
    if (request.id === undefined) return;
    let result = {};
    if (request.method === 'initialize') {
        result = {protocolVersion: '2024-11-05', capabilities: {tools: {}}, serverInfo: {name: 'craft-test', version: '1'}};
    } else if (request.method === 'tools/list') {
        result = {tools: ['core_target_triage', 'unexpected_fixture_tool'].map(name => ({name, description: name, inputSchema: {type: 'object', properties: {}}}))};
    }
    process.stdout.write(JSON.stringify({jsonrpc: '2.0', id: request.id, result}) + '\n');
});
"#,
        )
        .unwrap();
        let body = chat_sse(&[
            json!({"id":"test", "choices":[{"index":0,"delta":{"role":"assistant","content":"done"},"finish_reason":null}]}),
            json!({"id":"test", "choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
        ]);
        let (url, seen) = mock(vec![(200, "text/event-stream", body)]);
        let program = std::env::var("CRAFT_TEST_COPILOT").unwrap_or_else(|_| "copilot".into());
        let mut args = crate::cli_agents::adapters::Copilot.acp_options(&BackendOptions::default(), access);
        args.extend(["--acp".into(), "--stdio".into()]);
        let mcp = McpCommand { command: "node".into(), args: vec![script.display().to_string()], notes: None };
        let question = matches!(access, Access::Question | Access::CleanRoomQuestion);
        if !question {
            args.extend(crate::cli_agents::adapters::Copilot.acp_mcp_options(&mcp));
        }
        let mut conversation = AcpConversation::new("Copilot test", "copilot", program, args, directory.path().into(), mcp, None)
            .with_adapter(Arc::new(OfflineCopilot { url, home: directory.path().into() }))
            .with_question(question);
        conversation.send("Reply with done. Do not call any tools.");
        let started = Instant::now();
        let mut output = Vec::new();
        while started.elapsed() < Duration::from_secs(30) && !output.iter().any(|event| matches!(event, AgentEvent::TurnDone { .. })) {
            output.extend(conversation.poll());
            std::thread::park_timeout(Duration::from_millis(10));
        }
        assert!(output.iter().any(|event| matches!(event, AgentEvent::TurnDone { .. })), "{output:?}");
        assert!(!output.iter().any(|event| matches!(event, AgentEvent::Error { .. })), "{output:?}");
        let requests = seen.lock().unwrap();
        assert_eq!(requests.len(), 1, "{requests:?}");
        // Assert what reached the model, not what the CLI accepted on its command line:
        // startup can succeed while a tool is missing or an empty filter restores native tools.
        let mut tools: Vec<&str> = requests[0]
            .body
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
            .collect();
        tools.sort();
        expected.sort();
        assert_eq!(tools, expected, "{access:?}");
        assert_eq!(directory.path().join("mcp-requests.log").exists(), !question);
    }
}

/// #54, #98: a server on another computer is added by its address, with a model that can chat
/// when none is named; one that doesn't answer with its models isn't added.
#[test]
fn a_server_is_added_by_its_address() {
    let (h, _d) = hub();
    let url = serve("200 OK", r#"{"object":"list","data":[{"id":"nomic-embed-text"},{"id":"qwen3:8b"}]}"#);
    assert_eq!(h.add_server(" Studio box ", &format!("{url}/"), None, Some(" sk-test ".into())).unwrap(), "qwen3:8b");
    let b = h.backends().into_iter().find(|b| b.name == "Studio box").unwrap();
    assert_eq!((b.kind, b.model.as_str(), b.base_url.as_deref()), (BackendKind::OpenaiCompatible, "qwen3:8b", Some(url.as_str())));
    assert_eq!(h.add_server("Named", &url, Some("llama3"), None).unwrap(), "llama3", "a named model is kept");
    assert!(h.add_server("Web page", &serve("404 Not Found", "nope"), None, None).is_err(), "not a model server");
    assert!(h.add_server("Empty", &serve("200 OK", r#"{"object":"list","data":[]}"#), None, None).is_err(), "no models to use");
    let embeddings = serve("200 OK", r#"{"object":"list","data":[{"id":"nomic-embed-text"}]}"#);
    let error = h.add_server("Embeddings", &embeddings, None, None).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotFound);
    assert!(error.message.contains("model that can chat"));
    assert!(h.add_server("No scheme", "192.168.1.20:11434/v1", None, None).is_err());
    assert!(h.add_server(" ", &url, None, None).is_err());
    assert_eq!(h.backends().len(), 2, "only the ones that answered");
}

#[test]
fn a_backend_context_budget_reaches_the_model() {
    let (hub, dir) = hub();
    let table: toml::Table = toml::from_str(
        "[[backends]]\nname = \"Local\"\nkind = \"openai_compatible\"\nmodel = \"qwen3:8b\"\nbase_url = \"http://127.0.0.1:9/v1\"\ncontext_tokens = 8000\n",
    )
    .unwrap();
    let entry = AiConfig::from_table(&table).unwrap().backends.remove(0);
    hub.add_backend_with(entry, None).unwrap();
    let slot = hub.build_model(&hub.entry("Local").unwrap()).unwrap();
    assert_eq!(slot.context_tokens, 8000);
    let saved = AiConfig::from_settings(&Settings::load_from(&dir.path().join("config.toml")));
    assert_eq!(saved.get("Local").unwrap().options.context_tokens, Some(8000));
}

/// #374: a remote OpenAI-compatible service was treated as a local model: a 24K-token context
/// and a cost of nothing. Only a server on this computer or its network is local.
#[test]
fn only_an_openai_compatible_server_nearby_is_local() {
    let (h, _d) = hub();
    for (name, base, local) in [
        ("Ollama", "http://127.0.0.1:11434/v1", true),
        ("LM Studio box", "http://192.168.1.20:1234/v1", true),
        ("Docker Ollama", "http://ollama:11434/v1", true),
        ("IPv6 link-local", "http://[fe80::1]:11434/v1", true),
        ("Z.ai", "https://api.z.ai/api/coding/paas/v4", false),
        ("Public vLLM", "http://8.8.8.8:8000/v1", false),
    ] {
        add(&h, name, BackendKind::OpenaiCompatible, base, None);
        let slot = h.build_model(&h.entry(name).unwrap()).unwrap();
        assert_eq!(slot.local, local, "{name} at {base}");
    }
    h.add_backend(
        BackendConfig { name: "Default".into(), kind: BackendKind::OpenaiCompatible, model: String::new(), base_url: None, ssh_host: None, enabled: true },
        None,
    )
    .unwrap();
    assert!(h.build_model(&h.entry("Default").unwrap()).unwrap().local, "no address means Ollama on this computer");
    add(&h, "OpenAI", BackendKind::Openai, "http://127.0.0.1:9/v1", Some("sk-test"));
    assert!(!h.build_model(&h.entry("OpenAI").unwrap()).unwrap().local, "only OpenAI-compatible servers can be local");
}

fn add(h: &AiHub, name: &str, kind: BackendKind, base: &str, key: Option<&str>) {
    h.add_backend(
        BackendConfig { name: name.into(), kind, model: String::new(), base_url: Some(base.into()), ssh_host: None, enabled: true },
        key.map(str::to_string),
    )
    .unwrap();
}

fn run(c: &mut Box<dyn Conversation>, text: &str) -> Vec<AgentEvent> {
    c.send(text);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut all = Vec::new();
    while Instant::now() < deadline {
        for e in c.poll() {
            let done = matches!(e, AgentEvent::TurnDone { .. });
            all.push(e);
            if done {
                return all;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out: {all:?}");
}

#[test]
fn anthropic_streaming_tool_loop_through_the_engine() {
    let turn1 = sse(&[
        json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":100,"output_tokens":1}}}),
        json!({"type":"ping"}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"plan"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Checking"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"tu_1","name":"core_session_show","input":{}}}),
        json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{"}}),
        json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"}"}}),
        json!({"type":"content_block_stop","index":2}),
        json!({"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"tu_2","name":"core_targets_list","input":{}}}),
        json!({"type":"content_block_stop","index":3}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":40}}),
        json!({"type":"message_stop"}),
    ]);
    let turn2 = sse(&[
        json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":200,"cache_read_input_tokens":90}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" Nothing is open."}}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}),
        json!({"type":"message_stop"}),
    ]);
    let (url, seen) = mock(vec![(200, "text/event-stream", turn1), (200, "text/event-stream", turn2)]);
    let (h, _d) = hub();
    add(&h, "Claude", BackendKind::Anthropic, &url, Some("sk-test-123"));
    let mut c = h.start(None, true).unwrap();
    let ev = run(&mut c, "How does the session look?");
    let text: String = ev.iter().filter_map(|e| if let AgentEvent::TextDelta { text } = e { Some(text.as_str()) } else { None }).collect();
    assert_eq!(text, "Checking Nothing is open.");
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")));
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_targets_list")));
    let AgentEvent::TurnDone { backend, input_tokens, output_tokens, cost_usd, .. } = ev.last().unwrap().clone() else { panic!() };
    // The 90 cached tokens count as input, as agent apps report them.
    assert_eq!((backend.as_str(), input_tokens, output_tokens), ("Claude", 390, 50));
    assert!(cost_usd.unwrap() > 0.0);

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    let r1 = &seen[0];
    assert_eq!(r1.path, "/v1/messages");
    assert_eq!(r1.header("x-api-key"), Some("sk-test-123"));
    assert_eq!(r1.header("anthropic-version"), Some("2023-06-01"));
    // The fallback beta is only for Anthropic's own address, not this stand-in.
    assert_eq!(r1.header("anthropic-beta"), None);
    assert_eq!(r1.header("user-agent"), Some("CraftAnything-dev"));
    let b = &r1.body;
    assert_eq!(b["model"], "claude-opus-5-5");
    assert_eq!(b["stream"], true);
    assert_eq!(b["max_tokens"], 64000);
    assert_eq!(b["thinking"]["type"], "adaptive");
    assert_eq!(b["output_config"]["effort"], "high");
    assert!(b.get("fallbacks").is_none());
    assert_eq!(b["tool_choice"]["type"], "auto");
    assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
    assert!(b["system"][0]["text"].as_str().unwrap().contains("Investigation so far"));
    let tools = b["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "tool order is deterministic");
    assert!(names.contains(&"core_session_show") && names.contains(&"enable_toolset") && names.contains(&"core_handoff_write"));
    assert!(!names.contains(&"native_cursor_show"), "non-Core tools wait for enable_toolset");
    assert_eq!(tools.last().unwrap()["cache_control"]["type"], "ephemeral");
    assert!(tools.iter().all(|t| t["input_schema"]["type"] == "object"));
    // Turn 2 replays thinking (with signature) and returns BOTH results in ONE user message.
    let msgs = seen[1].body["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[1]["content"][0], json!({"type":"thinking","thinking":"plan","signature":"SIG"}));
    let results = msgs[2]["content"].as_array().unwrap();
    assert_eq!(msgs[2]["role"], "user");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["tool_use_id"], "tu_1");
    assert_eq!(results[1]["tool_use_id"], "tu_2");
    assert_eq!(results[0]["is_error"], false);
    assert!(results[0]["content"].as_str().unwrap().contains("deep"), "test sessions are in Deep mode");
    // the key is never stored in settings
    let saved = std::fs::read_to_string(_d.path().join("config.toml")).unwrap();
    assert!(!saved.contains("sk-test-123"));
}

#[test]
fn anthropic_refusal_and_http_errors() {
    let refusal = sse(&[
        json!({"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":5}}}),
        json!({"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":0}}),
        json!({"type":"message_stop"}),
    ]);
    let (url, _seen) = mock(vec![
        (200, "text/event-stream", refusal),
        (401, "application/json", r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.into()),
    ]);
    let (h, _d) = hub();
    add(&h, "Claude", BackendKind::Anthropic, &url, Some("k"));
    let mut c = h.start(Some("Claude"), false).unwrap();
    let ev = run(&mut c, "q");
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::Error { message, .. } if message.contains("declined"))));
    let ev = run(&mut c, "q2");
    assert!(
        ev.iter().any(|e| matches!(e, AgentEvent::Error { message, remediation: Some(r), .. } if message.contains("invalid x-api-key") && r.contains("key"))),
        "{ev:?}"
    );
}

/// A Claude client that learns capabilities reads the model list once before its first turn and
/// leaves out what the model says it doesn't take; a list it can't read means "unknown" (sent as
/// before) and isn't asked for again on the next turn.
#[test]
fn a_turn_learns_the_models_capabilities_once() {
    let answer = || {
        sse(&[
            json!({"type":"message_start","message":{"model":"plain","usage":{"input_tokens":3}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":"ok"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
            json!({"type":"message_stop"}),
        ])
    };
    let models = json!({"data": [{"id": "plain", "capabilities": {"effort": {"supported": false}, "thinking": {"supported": false}}}], "has_more": false});
    let msgs = vec![model::Msg::user_text("hi")];
    let req = model::TurnRequest { system: "s", messages: &msgs, tools: &[], effort: Effort::High, max_tokens: 10 };
    let stop = model::Stop::default();
    for listed in [true, false] {
        let list = if listed { (200, "application/json", models.to_string()) } else { (404, "application/json", r#"{"error":{"message":"no"}}"#.to_string()) };
        let (url, seen) = mock(vec![list, (200, "text/event-stream", answer()), (200, "text/event-stream", answer())]);
        let mut claude = anthropic::Anthropic::new(Some(&url), "sk-test".into(), "plain");
        assert!(!claude.learn_capabilities, "a stand-in address isn't asked unless told to");
        claude.learn_capabilities = true;
        model::ChatModel::turn(&mut claude, &req, &mut |_| {}, &stop).unwrap();
        model::ChatModel::turn(&mut claude, &req, &mut |_| {}, &stop).unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["/v1/models?limit=1000", "/v1/messages", "/v1/messages"], "listed={listed}");
        for turn in &seen[1..] {
            assert_eq!(turn.body.get("output_config").is_none(), listed, "{}", turn.body);
            assert_eq!(turn.body.get("thinking").is_none(), listed, "{}", turn.body);
        }
    }
}

/// A request id is kept for logs, not shown as advice.
#[test]
fn a_model_list_error_keeps_the_request_id_for_logs() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let body = r#"{"type":"error","error":{"type":"not_found_error","message":"nope"}}"#;
            let _ = write!(s, "HTTP/1.1 404 Not Found\r\nrequest-id: req_011Test\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
        }
    });
    let e = anthropic::Anthropic::new(Some(&url), "k".into(), "").list_models(std::time::Duration::from_secs(5)).unwrap_err();
    assert_eq!(e.details.as_ref().unwrap()["request_id"], "req_011Test");
    assert!(!e.remediation.unwrap_or_default().contains("req_011Test"));
}

#[test]
fn openai_streaming_tool_loop() {
    let t1 = chat_sse(&[
        json!({"model":"gpt-x","choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"core_session_show","arguments":""}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{}"}}]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":3}}),
    ]);
    let t2 = chat_sse(&[json!({"choices":[{"delta":{"content":"Nothing open."},"finish_reason":"stop"}]})]);
    let (url, seen) = mock(vec![(200, "text/event-stream", t1), (200, "text/event-stream", t2)]);
    let (h, _d) = hub();
    add(&h, "GPT", BackendKind::Openai, &url, Some("sk-o"));
    let mut c = h.start(Some("GPT"), true).unwrap();
    let ev = run(&mut c, "status?");
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::TextDelta { text } if text == "Nothing open.")));
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].path, "/chat/completions");
    assert_eq!(seen[0].header("authorization"), Some("Bearer sk-o"));
    assert_eq!(seen[0].body["stream_options"]["include_usage"], true);
    assert_eq!(seen[0].body["tool_choice"], "auto");
    let msgs = seen[1].body["messages"].as_array().unwrap();
    assert_eq!(msgs[2]["tool_calls"][0]["id"], "call_a");
    assert_eq!(msgs[3]["role"], "tool");
    assert_eq!(msgs[3]["tool_call_id"], "call_a");
}

#[test]
fn native_tool_stop_reasons_are_respected_before_execution() {
    for (finish, run_tool) in [("length", false), ("content_filter", false), ("stop", true), ("tool_calls", true)] {
        let first = chat_sse(&[
            json!({"choices":[{"delta":{"tool_calls":[{
                "index":0,"id":"call_a","type":"function","function":{"name":"core_session_show","arguments":"{}"}
            }]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":finish}]}),
        ]);
        let mut responses = vec![(200, "text/event-stream", first)];
        if finish != "content_filter" {
            let last = chat_sse(&[json!({"choices":[{"delta":{"content":"Done."},"finish_reason":"stop"}]})]);
            responses.push((200, "text/event-stream", last));
        }
        let (url, seen) = mock(responses);
        let (h, _dir) = hub();
        add(&h, "GPT", BackendKind::Openai, &url, Some("sk-test"));
        let mut conversation = h.start(Some("GPT"), false).unwrap();
        let events = run(&mut conversation, "status?");
        let started = events.iter().filter(|event| matches!(event, AgentEvent::ToolStarted { .. })).count();
        let completed = events.iter().filter(|event| matches!(event, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")).count();
        assert_eq!(started, usize::from(run_tool), "finish_reason={finish}: {events:?}");
        assert_eq!(completed, usize::from(run_tool), "finish_reason={finish}: {events:?}");
        let seen = seen.lock().unwrap();
        if finish == "content_filter" {
            assert_eq!(seen.len(), 1, "a refused turn must not request a continuation");
            assert!(events.iter().any(|event| matches!(event, AgentEvent::Error { message, .. } if message.contains("declined"))));
        } else {
            assert_eq!(seen.len(), 2);
            let messages = seen[1].body["messages"].as_array().unwrap();
            let result = messages.last().unwrap();
            assert_eq!(result["role"], "tool");
            assert_eq!(result["tool_call_id"], "call_a");
            assert_eq!(result["content"].as_str().unwrap().starts_with("Not run: the reply hit the output limit"), finish == "length");
            assert!(events.iter().any(|event| matches!(event, AgentEvent::TextDelta { text } if text == "Done.")));
        }
    }
}

fn assert_unexecuted_native_tool_calls_are_paired(finish: &str) {
    let first = chat_sse(&[
        json!({"choices":[{"delta":{"content":"Stopped before executing.","tool_calls":[
            {"index":0,"id":"call_a","type":"function","function":{"name":"core_session_show","arguments":"{}"}},
            {"index":1,"id":"call_b","type":"function","function":{"name":"core_session_show","arguments":"{}"}}
        ]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":finish}]}),
    ]);
    let last = chat_sse(&[json!({"choices":[{"delta":{"content":"Done."},"finish_reason":"stop"}]})]);
    let (url, seen) = mock(vec![(200, "text/event-stream", first), (200, "text/event-stream", last)]);
    let (h, _dir) = hub();
    add(&h, "GPT", BackendKind::Openai, &url, Some("sk-test"));
    let mut conversation = h.start(Some("GPT"), false).unwrap();
    let mut events = run(&mut conversation, "status?");
    if finish != "pause_turn" {
        assert_eq!(seen.lock().unwrap().len(), 1, "{finish} must wait for the next user turn");
        events.extend(run(&mut conversation, "Continue when ready."));
    }
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::ToolStarted { .. } | AgentEvent::ToolFinished { .. } | AgentEvent::ConsentNeeded { .. })),
        "stopped calls must not run or ask for consent: {events:?}"
    );
    assert!(!events.iter().any(|event| matches!(event, AgentEvent::Error { .. })), "{events:?}");
    assert!(events.iter().any(|event| matches!(event, AgentEvent::TextDelta { text } if text == "Done.")), "{events:?}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "{finish}: one stopped response and one continuation");
    let messages = seen[1].body["messages"].as_array().unwrap();
    let call_at = messages.iter().position(|message| message["tool_calls"].is_array()).unwrap();
    assert_eq!(messages[call_at]["tool_calls"].as_array().unwrap().len(), 2);
    assert_eq!(messages[call_at]["content"], "Stopped before executing.");
    for (offset, id) in ["call_a", "call_b"].into_iter().enumerate() {
        let result = messages.get(call_at + offset + 1);
        assert_eq!(result.and_then(|message| message["role"].as_str()), Some("tool"), "{finish}: each retained call needs a result: {messages:?}");
        let result = result.unwrap();
        assert_eq!(result["tool_call_id"], id);
        assert!(result["content"].as_str().unwrap().starts_with("Not run:"), "{result:?}");
    }
    if finish == "pause_turn" {
        assert_eq!(messages.last().unwrap()["tool_call_id"], "call_b", "a pause continues without another user request");
    } else {
        assert_eq!(messages.last().unwrap()["role"], "user");
        assert_eq!(messages.last().unwrap()["content"], "Continue when ready.");
    }
}

#[test]
fn paused_native_tool_calls_keep_history_paired() {
    assert_unexecuted_native_tool_calls_are_paired("pause_turn");
}

#[test]
fn sequence_stopped_native_tool_calls_keep_history_paired() {
    assert_unexecuted_native_tool_calls_are_paired("stop_sequence");
}

#[test]
fn provider_stopped_native_tool_calls_keep_history_paired() {
    assert_unexecuted_native_tool_calls_are_paired("provider_stop");
}

#[test]
fn local_server_without_tools_falls_back_to_react() {
    let t1 = chat_sse(&[json!({"choices":[{"delta":{"content":"Thought: check\nAction: core_session_show\nAction Input: {}"},"finish_reason":"stop"}]})]);
    let t2 = chat_sse(&[json!({"choices":[{"delta":{"content":"Final Answer: nothing is open"},"finish_reason":"stop"}]})]);
    let (url, seen) = mock(vec![
        (400, "application/json", r#"{"error":{"message":"registry.ollama.ai/library/tiny does not support tools"}}"#.into()),
        (200, "text/event-stream", t1),
        (200, "text/event-stream", t2),
    ]);
    let (h, _d) = hub();
    add(&h, "Tiny", BackendKind::OpenaiCompatible, &format!("{url}/v1"), None);
    let mut c = h.start(Some("Tiny"), true).unwrap();
    let ev = run(&mut c, "status?");
    assert!(ev.iter().any(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")), "{ev:?}");
    assert!(matches!(ev.last(), Some(AgentEvent::TurnDone { cost_usd: Some(c), .. }) if *c == 0.0));
    let seen = seen.lock().unwrap();
    assert!(seen[0].body.get("tools").is_some());
    assert!(seen[0].header("authorization").is_none());
    assert!(seen[1].body.get("tools").is_none());
    let last = seen[2].body["messages"].as_array().unwrap().last().unwrap().clone();
    assert!(last["content"].as_str().unwrap().starts_with("Observation:"));
}

/// #498: return a field-specific error to the model, then accept its corrected call.
#[test]
fn a_model_can_correct_an_array_sent_to_a_string_parameter() {
    let call = |id: &str, input: Value| {
        chat_sse(&[json!({"choices":[{
            "delta":{"tool_calls":[{"index":0,"id":id,"type":"function","function":{
                "name":"core_unknown_add","arguments":input.to_string()
            }}]},"finish_reason":"tool_calls"
        }]})])
    };
    let (url, seen) = mock(vec![
        (200, "text/event-stream", call("wrong", json!({"question": ["Where is the loop?"]}))),
        (200, "text/event-stream", call("corrected", json!({"question": "Where is the loop?"}))),
        (200, "text/event-stream", chat_sse(&[json!({"choices":[{"delta":{"content":"Recorded."},"finish_reason":"stop"}]})])),
    ]);
    let (h, _d) = hub();
    add(&h, "Local", BackendKind::OpenaiCompatible, &url, None);
    let mut c = h.start(Some("Local"), true).unwrap();
    let events = run(&mut c, "Record what remains unknown.");
    let completed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolFinished { tool, ok, .. } if tool == "core_unknown_add" => Some(*ok),
            _ => None,
        })
        .collect();
    assert_eq!(completed, vec![false, true]);
    assert_eq!(h.engine.session().unknowns().count(), 1, "only the corrected call creates a record");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let result = seen[1].body["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(result["role"], "tool");
    assert_eq!(result["tool_call_id"], "wrong");
    let error: Value = serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "bad_input");
    assert_eq!(error["error"]["details"]["parameter_path"], "/question");
    assert!(error["error"]["message"].as_str().unwrap().contains("expected a string"));
    assert!(error["error"]["remediation"].as_str().unwrap().contains("call the tool again"));
}

#[test]
fn json_text_tool_calls_execute_through_both_protocols() {
    for react in [false, true] {
        for call in [
            r#"{"name":"core_session_show","arguments":{}}"#,
            "```json\n{\"name\":\"core_session_show\",\"arguments\":{}}\n````",
            "````json\n{\"name\":\"core_session_show\",\"arguments\":{}}\n````",
        ] {
            let t1 = chat_sse(&[json!({"choices":[{"delta":{"content":call},"finish_reason":"stop"}]})]);
            let t2 = chat_sse(&[json!({"choices":[{"delta":{"content":"Nothing open."},"finish_reason":"stop"}]})]);
            let (url, seen) = mock(vec![(200, "text/event-stream", t1), (200, "text/event-stream", t2)]);
            let (hub, _dir) = hub();
            let config = BackendConfig {
                name: "Local".into(),
                kind: BackendKind::OpenaiCompatible,
                model: "scripted".into(),
                base_url: Some(url),
                ssh_host: None,
                enabled: true,
            };
            hub.add_backend_with(BackendEntry { config, options: BackendOptions { react_fallback: Some(react), ..Default::default() } }, None).unwrap();
            let mut conversation = hub.start(Some("Local"), true).unwrap();
            let events = run(&mut conversation, "status?");
            assert_eq!(
                events.iter().filter(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")).count(),
                1,
                "react={react}, call={call}: {events:?}"
            );
            assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta { text } if text == "Nothing open.")), "{events:?}");
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 2);
            let messages = seen[1].body["messages"].as_array().unwrap();
            let result = messages.last().unwrap();
            if react {
                assert_eq!(result["role"], "user");
                assert!(result["content"].as_str().unwrap().starts_with("Observation:"));
            } else {
                assert_eq!(result["role"], "tool");
                assert_eq!(result["tool_call_id"], messages[2]["tool_calls"][0]["id"]);
                assert_eq!(messages[2]["tool_calls"][0]["function"]["name"], "core_session_show");
            }
        }
    }
}

#[test]
fn react_action_errors_still_reach_the_tool_loop() {
    let replies = ["Action: missing_tool\nAction Input: {}", "Action: core_session_show\nAction Input: {}", "Final Answer: Nothing open."]
        .map(|text| (200, "text/event-stream", chat_sse(&[json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]})])));
    let (url, seen) = mock(replies.into());
    let (hub, _dir) = hub();
    let config = BackendConfig {
        name: "Local".into(),
        kind: BackendKind::OpenaiCompatible,
        model: "scripted".into(),
        base_url: Some(url),
        ssh_host: None,
        enabled: true,
    };
    hub.add_backend_with(BackendEntry { config, options: BackendOptions { react_fallback: Some(true), ..Default::default() } }, None).unwrap();
    let mut conversation = hub.start(Some("Local"), true).unwrap();
    let events = run(&mut conversation, "status?");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: false, .. } if tool == "missing_tool")), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")), "{events:?}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let correction = seen[1].body["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap();
    assert!(correction.starts_with("Observation (error):") && correction.contains("No tool named missing_tool"), "{correction}");
}

#[test]
fn fenced_legacy_react_calls_still_execute() {
    for call in [
        "```text\nAction: core_session_show\nAction Input: {}\n```",
        "```text\nAction: core_session_show\nAction Input: {}\n````",
        "````text\nAction: core_session_show\nAction Input: {}\n````",
        "````text\nAction: core_session_show\nAction Input: ```json\n{}\n```\n````",
    ] {
        let replies = [call, "Final Answer: Nothing open."]
            .map(|text| (200, "text/event-stream", chat_sse(&[json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]})])));
        let (url, seen) = mock(replies.into());
        let (hub, _dir) = hub();
        let config = BackendConfig {
            name: "Local".into(),
            kind: BackendKind::OpenaiCompatible,
            model: "scripted".into(),
            base_url: Some(url),
            ssh_host: None,
            enabled: true,
        };
        hub.add_backend_with(BackendEntry { config, options: BackendOptions { react_fallback: Some(true), ..Default::default() } }, None).unwrap();
        let mut conversation = hub.start(Some("Local"), true).unwrap();
        let events = run(&mut conversation, "status?");
        assert_eq!(
            events.iter().filter(|e| matches!(e, AgentEvent::ToolFinished { tool, ok: true, .. } if tool == "core_session_show")).count(),
            1,
            "the fenced legacy call must execute exactly once: {events:?}"
        );
        assert!(events.iter().any(|e| matches!(e, AgentEvent::TextDelta { text } if text == "Final Answer: Nothing open.")), "{events:?}");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        let observation = seen[1].body["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap();
        assert!(observation.starts_with("Observation:"), "{observation}");
    }
}

#[test]
fn json_text_recovery_preserves_non_actions_and_stop_reasons() {
    use crate::model::{Effort, Msg, Stop, StopReason, ToolDef, TurnRequest};
    let tools = [ToolDef { name: "look".into(), description: "Look at a file".into(), schema: json!({"type":"object"}) }];
    let messages = [Msg::user_text("check")];
    let request = TurnRequest { system: "S", messages: &messages, tools: &tools, effort: Effort::Low, max_tokens: 100 };
    for react in [false, true] {
        for (text, finish, expected) in [
            (r#"{"name":"unknown","arguments":{}}"#, "stop", StopReason::EndTurn),
            (r#"{"name":"look","arguments":{}}"#, "length", StopReason::MaxTokens),
            (r#"{"name":"look","arguments":{}}"#, "content_filter", StopReason::Refusal),
            (r#"{"name":"look","arguments":{}"#, "stop", StopReason::EndTurn),
            (r#"{"answer":42}"#, "stop", StopReason::EndTurn),
            (r#"For example: {"name":"look","arguments":{}}"#, "stop", StopReason::EndTurn),
            ("```text\nAction: look\nAction Input: {}\n```not a close```", "stop", StopReason::EndTurn),
            ("```json\n{\"name\":\"look\",\"arguments\":{}}```", "stop", StopReason::EndTurn),
            ("```text\nAction: look\nAction Input: {}```", "stop", StopReason::EndTurn),
            ("````text\nAction: look\nAction Input: {}\n```", "stop", StopReason::EndTurn),
            ("```text\nAction: look\nAction Input: {}\n```\nThis is only an example.\n```", "stop", StopReason::EndTurn),
        ] {
            let reply = chat_sse(&[json!({"choices":[{"delta":{"content":text},"finish_reason":finish}]})]);
            let (url, _seen) = mock(vec![(200, "text/event-stream", reply)]);
            let mut model = openai::OpenAi::compatible("Local", &url, None, "scripted");
            model.react = react;
            let result = model.turn(&request, &mut |_| {}, &Stop::default()).unwrap();
            assert!(result.message.tool_uses().is_empty(), "{result:?}");
            assert_eq!(result.stop, expected, "{text}");
        }
    }
    let call = r#"{"name":"look","arguments":{}}"#;
    for (automatic, offered) in [(false, tools.as_slice()), (true, &[])] {
        let reply = chat_sse(&[json!({"choices":[{"delta":{"content":call},"finish_reason":"stop"}]})]);
        let (url, _seen) = mock(vec![(200, "text/event-stream", reply)]);
        let mut model = openai::OpenAi::compatible("Local", &url, None, "scripted");
        model.auto_react = automatic;
        let request = TurnRequest { system: "S", messages: &messages, tools: offered, effort: Effort::Low, max_tokens: 100 };
        assert!(model.turn(&request, &mut |_| {}, &Stop::default()).unwrap().message.tool_uses().is_empty());
    }
}

#[test]
fn native_tool_calls_are_not_duplicated_by_json_text() {
    use crate::model::{Effort, Msg, Stop, StopReason, ToolDef, TurnRequest};
    let tools = [ToolDef { name: "look".into(), description: "Look at a file".into(), schema: json!({"type":"object"}) }];
    let messages = [Msg::user_text("check")];
    let request = TurnRequest { system: "S", messages: &messages, tools: &tools, effort: Effort::Low, max_tokens: 100 };
    for react in [false, true] {
        let reply = chat_sse(&[json!({"choices":[{"delta":{
            "content":r#"{"name":"look","arguments":{}}"#,
            "tool_calls":[{"index":0,"id":"native_call","type":"function","function":{"name":"look","arguments":"{}"}}]
        },"finish_reason":"tool_calls"}]})]);
        let (url, _seen) = mock(vec![(200, "text/event-stream", reply)]);
        let mut model = openai::OpenAi::compatible("Local", &url, None, "scripted");
        model.react = react;
        let result = model.turn(&request, &mut |_| {}, &Stop::default()).unwrap();
        assert_eq!(result.message.tool_uses(), vec![("native_call".into(), "look".into(), json!({}))]);
        assert_eq!(result.stop, StopReason::ToolUse);
    }
}

#[test]
fn hub_config_keys_and_test_backend() {
    let t = sse(&[
        json!({"type":"message_start","message":{"model":"claude-haiku-5-5","usage":{"input_tokens":3}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":"ready"}}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
        json!({"type":"message_stop"}),
    ]);
    let models = json!({"data":[{"id":"claude-opus-5-5"},{"id":"claude-haiku-5-5"}]}).to_string();
    let (url, seen) = mock(vec![(200, "text/event-stream", t), (200, "application/json", models.clone()), (200, "application/json", models)]);
    let (h, d) = hub();
    assert!(h.backends().is_empty());
    assert!(h.start(None, true).is_err());
    add(&h, "Claude", BackendKind::Anthropic, &url, Some("k1"));
    add(&h, "Local", BackendKind::OpenaiCompatible, "http://127.0.0.1:9/v1", None);
    assert_eq!(h.backends().len(), 2);
    assert!(h.has_key("Claude") && h.has_key("Local"));
    let r = h.test_backend("Claude").unwrap();
    assert_eq!((r.reply.as_str(), r.version_only), ("ready", false));
    assert_eq!(r.model, "claude-haiku-5-5");
    assert_eq!(seen.lock().unwrap()[0].body["output_config"]["effort"], "high");
    assert_eq!(h.list_models("Claude").unwrap(), vec!["claude-opus-5-5", "claude-haiku-5-5"]);
    let reach = h.check_reachability();
    assert_eq!(reach.len(), 2);
    assert!(reach[0].1.is_ok(), "{reach:?}");
    assert!(reach[1].1.is_err());
    // settings persisted to the file; reload shows both
    let s = Settings::load_from(&d.path().join("config.toml"));
    assert_eq!(AiConfig::from_settings(&s).backends.len(), 2);
    assert!(h.remove_backend("Claude").unwrap());
    assert!(!h.remove_backend("Claude").unwrap());
    assert!(h.test_backend("Claude").is_err());
    // missing key is a clear error
    h.add_backend(
        BackendConfig {
            name: "NoKey".into(),
            kind: BackendKind::Openrouter,
            model: String::new(),
            base_url: Some("http://127.0.0.1:9".into()),
            ssh_host: None,
            enabled: true,
        },
        None,
    )
    .unwrap();
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        let e = h.start(Some("NoKey"), true).err().unwrap();
        assert_eq!(e.code, ErrorCode::BadSettings);
    }
    assert!(h.compare("Local", "Local", true).is_err());
}

/// #369: when OpenRouter has no provider that supports tools for a model (404 once only those are
/// asked for), the turn goes on with the text protocol instead of failing; the first request
/// required parameters, the second sends no tools.
#[test]
fn openrouter_with_no_provider_for_tools_falls_back_to_the_text_protocol() {
    use crate::model::{Effort, Msg, Stop, ToolDef, TurnRequest};
    let tools = [ToolDef { name: "look".into(), description: "Look at a file".into(), schema: json!({"type":"object"}) }];
    let messages = [Msg::user_text("hi")];
    let request = TurnRequest { system: "S", messages: &messages, tools: &tools, effort: Effort::Low, max_tokens: 100 };
    let none = json!({"error": {"code": 404, "message": "No endpoints found that can handle the requested parameters."}}).to_string();
    let reply = chat_sse(&[json!({"choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]})]);
    let (url, seen) = mock(vec![(404, "application/json", none), (200, "text/event-stream", reply)]);
    let mut model = openai::OpenAi::openrouter(Some(&url), "k".into(), "moonshotai/kimi-k3");
    let result = model.turn(&request, &mut |_| {}, &Stop::default()).unwrap();
    assert_eq!(result.message.text(), "hello");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "one retry");
    assert_eq!(seen[0].body["provider"], json!({"require_parameters": true}));
    assert!(seen[0].body.get("tools").is_some());
    assert!(seen[1].body.get("provider").is_none() && seen[1].body.get("tools").is_none(), "the second asks in text: {}", seen[1].body);
}
