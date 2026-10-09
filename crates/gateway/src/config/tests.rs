use std::{fs, os::unix::fs::PermissionsExt};

use serde_json::json;

use super::McpConfig;
use crate::SessionAccess;

fn access() -> SessionAccess {
    SessionAccess {
        url: "http://127.0.0.1:4321/mcp".into(),
        token: "secret-token".into(),
    }
}

#[test]
fn the_file_holds_the_http_server_with_its_bearer_token() {
    let dir = tempfile::tempdir().unwrap();
    let config = McpConfig::write(dir.path(), &access()).unwrap();
    let text = fs::read_to_string(config.path()).unwrap();
    let read: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        read,
        json!({ "mcpServers": { "atelier": {
            "type": "http",
            "url": "http://127.0.0.1:4321/mcp",
            "headers": { "Authorization": "Bearer secret-token" }
        } } })
    );
}

#[test]
fn the_file_is_private_to_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let config = McpConfig::write(dir.path(), &access()).unwrap();
    let mode = fs::metadata(config.path()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn the_folder_it_makes_is_private_too() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("run");
    let config = McpConfig::write(&inner, &access()).unwrap();
    let mode = fs::metadata(&inner).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    drop(config);
}

#[test]
fn the_file_is_removed_when_the_config_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let config = McpConfig::write(dir.path(), &access()).unwrap();
    let path = config.path().to_path_buf();
    assert!(path.exists());
    drop(config);
    assert!(!path.exists());
}

#[test]
fn two_sessions_get_two_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = McpConfig::write(dir.path(), &access()).unwrap();
    let b = McpConfig::write(dir.path(), &access()).unwrap();
    assert_ne!(a.path(), b.path());
}

#[test]
fn the_name_does_not_show_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let config = McpConfig::write(dir.path(), &access()).unwrap();
    assert!(!config.path().to_string_lossy().contains("secret-token"));
}
