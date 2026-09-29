//! `gh` itself is stood in for by a shell script that prints what the real one prints, as probed on
//! `gh` 2.101: a reply with `--include`, exit 4 when signed out, exit 1 with nothing when offline.
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};

use lathe_project::LocalProject;

use super::{GhCli, parse_reply};
use crate::github::transport::{Request, Transport, TransportError};

fn stand_in(script: &str) -> (tempfile::TempDir, GhCli) {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let cli = GhCli::new(project).with_program(path.to_string_lossy());
    (dir, cli)
}

fn get(path: &str) -> Request {
    Request { method: "GET", path: path.into(), body: None }
}

#[test]
fn a_reply_is_split_into_status_headers_and_body() {
    let text = "HTTP/2.0 200 OK\r\nContent-Type: application/json\r\nX-RateLimit-Remaining: 4999\r\n\r\n{\"a\":1}";
    let reply = parse_reply(text).unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("x-ratelimit-remaining"), Some("4999"));
    assert_eq!(reply.header("content-type"), Some("application/json"));
    assert_eq!(reply.body, "{\"a\":1}");
}

#[test]
fn an_error_reply_keeps_its_body() {
    let reply = parse_reply("HTTP/2.0 404 Not Found\nServer: github.com\n\n{\"message\":\"Not Found\"}").unwrap();
    assert_eq!((reply.status, reply.body.as_str()), (404, "{\"message\":\"Not Found\"}"));
}

#[test]
fn text_that_is_not_a_reply_is_none() {
    for text in ["", "gh: Bad credentials", "HTTP/2.0", "HTTP/2.0 abc\n\n"] {
        assert!(parse_reply(text).is_none(), "{text:?}");
    }
}

#[test]
fn a_reply_with_no_headers_after_the_status_line_still_parses() {
    let reply = parse_reply("HTTP/1.1 204 No Content\r\n\r\n").unwrap();
    assert_eq!((reply.status, reply.body.as_str()), (204, ""));
}

#[test]
fn the_request_runs_gh_api_with_include_and_sends_the_body_on_stdin() {
    let log = tempfile::tempdir().unwrap();
    let args = log.path().join("args");
    let input = log.path().join("input");
    let script = format!(
        "echo \"$@\" > '{}'\ncat > '{}'\nprintf 'HTTP/2.0 200 OK\\r\\n\\r\\n{{}}'",
        args.display(),
        input.display()
    );
    let (_dir, cli) = stand_in(&script);
    let request = Request { method: "POST", path: "graphql".into(), body: Some("{\"query\":\"query X {}\"}".into()) };
    assert_eq!(cli.send(&request).unwrap().status, 200);
    assert_eq!(std::fs::read_to_string(args).unwrap().trim(), "api --include --method POST --input - graphql");
    assert_eq!(std::fs::read_to_string(input).unwrap(), "{\"query\":\"query X {}\"}");
}

#[test]
fn a_get_sends_no_input_flag() {
    let log = tempfile::tempdir().unwrap();
    let args = log.path().join("args");
    let (_dir, cli) = stand_in(&format!("echo \"$@\" > '{}'\nprintf 'HTTP/2.0 200 OK\\r\\n\\r\\n'", args.display()));
    cli.send(&get("repos/o/r")).unwrap();
    assert_eq!(std::fs::read_to_string(args).unwrap().trim(), "api --include --method GET repos/o/r");
}

#[test]
fn exit_four_with_no_reply_means_signed_out() {
    let (_dir, cli) = stand_in("exit 4");
    assert_eq!(cli.send(&get("x")).err().unwrap(), TransportError::NotSignedIn);
}

#[test]
fn exit_one_with_no_reply_means_the_connection_never_opened() {
    let (_dir, cli) = stand_in("echo 'connection refused' >&2\nexit 1");
    assert_eq!(cli.send(&get("x")).err().unwrap(), TransportError::Offline);
}

#[test]
fn any_other_exit_with_no_reply_is_a_failure_with_its_code() {
    let (_dir, cli) = stand_in("exit 9");
    assert!(matches!(cli.send(&get("x")).err().unwrap(), TransportError::Failed(text) if text.contains("9")));
}

#[test]
fn a_gh_that_is_not_installed_is_reported_as_missing() {
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let cli = GhCli::new(project).with_program("/nonexistent/gh-for-lathe-tests");
    assert_eq!(cli.send(&get("x")).err().unwrap(), TransportError::ToolMissing);
}

#[test]
fn a_400_reply_with_a_body_is_a_reply_not_a_transport_failure() {
    let (_dir, cli) = stand_in("printf 'HTTP/2.0 401 Unauthorized\\r\\n\\r\\n{\"message\":\"Bad credentials\"}'\nexit 1");
    let reply = cli.send(&get("x")).unwrap();
    assert_eq!(reply.status, 401);
}
