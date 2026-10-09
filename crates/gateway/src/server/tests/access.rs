//! Who may talk to the gateway: the token, the Host, the Origin, the verb, and the end of a session.
use std::net::TcpStream;

use serde_json::json;

use super::{fixture, raw};

fn ping() -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })
}

#[test]
fn no_token_is_refused() {
    let f = fixture();
    let (status, _) = f.post(None, &[], &ping());
    assert_eq!(status, 401);
}

#[test]
fn a_wrong_token_is_refused() {
    let f = fixture();
    let (status, _) = f.post(Some("not-a-token"), &[], &ping());
    assert_eq!(status, 401);
}

#[test]
fn a_revoked_token_is_refused() {
    let f = fixture();
    let (before, _) = f.post(Some(&f.access.token), &[], &ping());
    assert_eq!(before, 200);
    f.gateway.revoke(&f.access.token);
    let (after, _) = f.post(Some(&f.access.token), &[], &ping());
    assert_eq!(after, 401);
}

#[test]
fn each_session_has_its_own_token() {
    let f = fixture();
    let other = f.gateway.session(super::agent()).unwrap();
    assert_ne!(other.token, f.access.token);
    assert_eq!(other.url, f.access.url);
}

#[test]
fn a_foreign_origin_is_refused() {
    let f = fixture();
    let (status, _) = f.post(
        Some(&f.access.token),
        &[("Origin", "https://evil.example")],
        &ping(),
    );
    assert_eq!(status, 403);
}

#[test]
fn a_loopback_origin_of_another_port_is_refused() {
    // A page served from this machine is still a page, not an agent.
    let f = fixture();
    let (status, _) = f.post(
        Some(&f.access.token),
        &[("Origin", "http://localhost:3000")],
        &ping(),
    );
    assert_eq!(status, 403);
}

#[test]
fn a_host_that_is_not_loopback_is_refused() {
    let f = fixture();
    let body = ping().to_string();
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: rebind.example\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        f.access.token,
        body.len()
    );
    let answer = raw(&f.access.url, &request);
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
}

#[test]
fn get_is_not_allowed() {
    let f = fixture();
    let mut response = f
        .http
        .get(&f.access.url)
        .header("Authorization", format!("Bearer {}", f.access.token))
        .call()
        .unwrap();
    assert_eq!(response.status().as_u16(), 405);
    assert_eq!(
        response
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok()),
        Some("POST")
    );
    response.body_mut().read_to_string().unwrap();
}

#[test]
fn another_path_is_not_found() {
    let f = fixture();
    let url = f.access.url.replace("/mcp", "/other");
    let response = f
        .http
        .post(&url)
        .header("Authorization", format!("Bearer {}", f.access.token))
        .send("{}")
        .unwrap();
    assert_eq!(response.status().as_u16(), 404);
}

#[test]
fn the_server_listens_on_loopback_only() {
    let f = fixture();
    assert!(
        f.access.url.starts_with("http://127.0.0.1:"),
        "{}",
        f.access.url
    );
}

#[test]
fn dropping_the_gateway_stops_the_server() {
    let f = fixture();
    let address = f
        .access
        .url
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
        .unwrap()
        .to_string();
    assert!(TcpStream::connect(&address).is_ok());
    drop(f);
    assert!(TcpStream::connect(&address).is_err(), "the port is closed");
}

#[test]
fn a_grant_ends_the_token_and_the_file_together() {
    let f = fixture();
    let dir = tempfile::tempdir().unwrap();
    let grant = f.gateway.grant(super::agent(), dir.path()).unwrap();
    let path = grant.config_path().to_path_buf();
    let token = grant.access().token.clone();
    assert!(std::fs::read_to_string(&path).unwrap().contains(&token));
    let (live, _) = f.post(Some(&token), &[], &ping());
    assert_eq!(live, 200);
    drop(grant);
    let (gone, _) = f.post(Some(&token), &[], &ping());
    assert_eq!(gone, 401);
    assert!(!path.exists());
}
