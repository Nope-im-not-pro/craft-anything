//! `ai providers`, `ai add --provider` and `ai key`, run as the built binary with a temp config
//! folder and no keychain. Nothing here reaches the network: adding uses `--no-check`, and the
//! checked paths point at a port that refuses every connection.

use craft_anything_platform::process::{Output, Process};
use serde_json::Value;
use std::time::Duration;

fn cli(args: &[&str], config: &std::path::Path, stdin: &[u8]) -> Output {
    Process::new(env!("CARGO_BIN_EXE_craft-anything-cli"))
        .args(args)
        .env("CRAFT_ANYTHING_CONFIG_DIR", config)
        .env("CRAFT_ANYTHING_NO_KEYCHAIN", "1")
        .stdin_bytes(stdin.to_vec())
        .timeout(Duration::from_secs(60))
        .run()
        .unwrap()
}

#[test]
fn the_claude_api_is_added_from_the_cli_with_a_key_on_stdin() {
    let cfg = tempfile::tempdir().unwrap();
    let out = cli(&["ai", "providers", "--format", "json"], cfg.path(), b"");
    assert!(out.success, "{}", String::from_utf8_lossy(&out.stderr));
    let providers: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(providers.as_array().unwrap().iter().any(|p| p["id"] == "anthropic" && p["needs_key"] == true), "{providers}");

    let out = cli(&["ai", "add", "Claude", "--provider", "anthropic", "--key-stdin", "--no-check", "--format", "json"], cfg.path(), b"sk-test-cli\n");
    assert!(out.success, "{}", String::from_utf8_lossy(&out.stderr));
    let added: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!((added["added"]["kind"].as_str(), added["added"]["key_stored"].as_bool()), (Some("anthropic"), Some(true)), "{added}");
    assert!(!String::from_utf8_lossy(&out.stdout).contains("sk-test-cli"));
    let keys = std::fs::read_to_string(cfg.path().join("ai-keys.toml")).unwrap();
    assert!(keys.contains("Claude = \"sk-test-cli\""), "the key went to the key file");
    let saved = std::fs::read_to_string(cfg.path().join("config.toml")).unwrap();
    assert!(saved.contains("kind = \"anthropic\"") && !saved.contains("sk-test-cli"));

    let list: Value = serde_json::from_slice(&cli(&["ai", "list", "--format", "json"], cfg.path(), b"").stdout).unwrap();
    assert_eq!(list[0]["key_stored"], true, "{list}");

    // The same name again is refused unless replaced, and a replace without a key keeps none.
    let again = ["ai", "add", "Claude", "--provider", "anthropic", "--base-url", "http://127.0.0.1:0", "--no-check"];
    assert!(!cli(&again, cfg.path(), b"").success);
    assert!(std::fs::read_to_string(cfg.path().join("ai-keys.toml")).unwrap().contains("sk-test-cli"));
    assert!(cli(&[&again[..], &["--replace"]].concat(), cfg.path(), b"").success);
    assert!(!std::fs::read_to_string(cfg.path().join("ai-keys.toml")).unwrap_or_default().contains("sk-test-cli"));
    assert!(cli(&["ai", "add", "Claude", "--provider", "anthropic", "--key-stdin", "--no-check", "--replace"], cfg.path(), b"sk-test-cli\n").success);
    let saved = std::fs::read_to_string(cfg.path().join("config.toml")).unwrap();

    // A checked key with no answer keeps the old one, and a bad key is refused before anything.
    let before = std::fs::read(cfg.path().join("ai-keys.toml")).unwrap();
    let moved = saved.replacen("kind = \"anthropic\"", "kind = \"anthropic\"\nbase_url = \"http://127.0.0.1:0\"", 1);
    std::fs::write(cfg.path().join("config.toml"), moved).unwrap();
    assert!(!cli(&["ai", "key", "Claude", "--key-stdin"], cfg.path(), b"sk-test-new").success);
    assert!(!cli(&["ai", "key", "Claude", "--key-stdin", "--no-check"], cfg.path(), "sk-tëst".as_bytes()).success);
    assert_eq!(std::fs::read(cfg.path().join("ai-keys.toml")).unwrap(), before);
    assert!(cli(&["ai", "key", "Claude", "--key-stdin", "--no-check"], cfg.path(), b"sk-test-new").success);
    assert!(std::fs::read_to_string(cfg.path().join("ai-keys.toml")).unwrap().contains("sk-test-new"));

    // The old way still works, `gemini` is a kind, and an unknown kind names them all.
    assert!(cli(&["ai", "add", "G", "--kind", "gemini", "--model", "gemini-3.8-flash"], cfg.path(), b"").success);
    let bad = cli(&["ai", "add", "X", "--kind", "nope"], cfg.path(), b"");
    assert_eq!(bad.code, Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("gemini"), "{}", String::from_utf8_lossy(&bad.stderr));
    assert_eq!(cli(&["ai", "add", "Y"], cfg.path(), b"").code, Some(2), "a kind or a provider is needed");
}
