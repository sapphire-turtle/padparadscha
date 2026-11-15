//! §4.1: the two app settings and local default; §10: a clear API-version refusal.

use padparadscha_common::{API_VERSION, ServerMode, Settings, check_api_version};
use std::path::Path;

#[test]
fn the_two_settings_and_the_local_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "[acp]\nurl = \"wss://agent.example/acp\"\n[server]\nmode = \"remote\"\n",
    )
    .unwrap();
    let s = Settings::load(Path::new(dir.path()));
    assert_eq!(s.acp_url, "wss://agent.example/acp");
    assert_eq!(s.server_mode, ServerMode::Remote);
}

#[test]
fn a_version_that_does_not_match_is_refused_with_a_clear_message() {
    assert!(check_api_version(API_VERSION).is_ok());
    let refused = check_api_version("0.0.1").expect_err("a mismatch is a mismatch");
    assert!(refused.to_string().contains(API_VERSION) && refused.to_string().contains("0.0.1"));
}

#[test]
fn absent_invalid_and_empty_settings_use_the_local_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let expected = Settings::default();
    assert_eq!(expected.acp_url, "ws://127.0.0.1:0/acp");
    assert_eq!(expected.server_mode, ServerMode::Local);
    assert_eq!(Settings::load(dir.path()), expected);
    for text in [
        "",
        "not valid toml",
        "[acp]\n",
        "[server]\nmode = \"local\"\n",
    ] {
        std::fs::write(dir.path().join("settings.toml"), text).unwrap();
        assert_eq!(Settings::load(dir.path()), expected, "input: {text}");
    }
}

#[test]
fn the_agent_url_does_not_require_a_server_table() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.toml"),
        "[acp]\nurl = \"wss://agent.example/acp\"\n",
    )
    .unwrap();
    let settings = Settings::load(dir.path());
    assert_eq!(settings.acp_url, "wss://agent.example/acp");
    assert_eq!(settings.server_mode, ServerMode::Local);
}
