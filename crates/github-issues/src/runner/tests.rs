//! `gh` itself is stood in for by a shell script that prints what the real one prints: a reply with `--include`, exit
//! 4 when signed out, exit 1 with a message when offline.
use std::{os::unix::fs::PermissionsExt, path::PathBuf};

use super::{
    Call, Failure, Gh, GhCli,
    helpers::{classify, parse_reply},
};

fn stand_in(script: &str) -> (tempfile::TempDir, GhCli) {
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A program that another test thread still holds open for writing will not start ("Text file busy"). Start it once
    // with nothing to read until it does; its output is not used.
    for _ in 0..200 {
        match Command::new(&path)
            .arg("--probe")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
            _ => break,
        }
    }
    let cli = GhCli::default().with_program(path.to_string_lossy());
    (dir, cli)
}

use std::process::{Command, Stdio};

#[test]
fn a_reply_is_split_into_status_headers_and_body() {
    let text = "HTTP/2.0 200 OK\r\nEtag: W/\"a\"\r\nX-RateLimit-Remaining: 4999\r\n\r\n{\"a\":1}";
    let reply = parse_reply(text).unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("x-ratelimit-remaining"), Some("4999"));
    assert_eq!(reply.header("etag"), Some("W/\"a\""));
    assert_eq!(reply.body, "{\"a\":1}");
}

#[test]
fn an_error_reply_keeps_its_body_and_a_304_has_none() {
    let reply = parse_reply("HTTP/2.0 404 Not Found\n\n{\"message\":\"Not Found\"}").unwrap();
    assert_eq!(
        (reply.status, reply.body.as_str()),
        (404, "{\"message\":\"Not Found\"}")
    );
    let reply = parse_reply("HTTP/2.0 304 Not Modified\r\nEtag: x\r\n\r\n").unwrap();
    assert_eq!((reply.status, reply.body.as_str()), (304, ""));
}

#[test]
fn text_that_is_not_a_reply_is_none() {
    for text in ["", "gh: Bad credentials", "HTTP/2.0", "HTTP/2.0 abc\n\n"] {
        assert!(parse_reply(text).is_none(), "{text:?}");
    }
}

#[test]
fn stderr_and_the_exit_code_tell_signed_out_from_offline() {
    assert_eq!(classify(Some(4), ""), Failure::NotSignedIn);
    assert_eq!(
        classify(
            Some(1),
            "To get started with GitHub CLI, please run:  gh auth login"
        ),
        Failure::NotSignedIn
    );
    assert_eq!(
        classify(
            Some(1),
            "error connecting to api.github.com\ncheck your internet connection"
        ),
        Failure::Offline
    );
    assert_eq!(
        classify(Some(1), "boom\n\n"),
        Failure::Other("gh ended with Some(1): boom".into())
    );
}

#[test]
fn gh_is_called_with_include_the_method_the_etag_and_the_body_on_stdin() {
    let (_dir, cli) =
        stand_in(r#"printf 'HTTP/2.0 201 Created\r\nEtag: e\r\n\r\n'; echo "$@"; cat"#);
    let call = Call::post("/repos/o/r/issues", serde_json::json!({"title": "T"}))
        .with_etag(Some("W/\"1\"".into()));
    let reply = cli.send(&call).unwrap();
    assert_eq!(reply.status, 201);
    let mut lines = reply.body.lines();
    assert_eq!(
        lines.next().unwrap(),
        "api --include --method POST -H If-None-Match: W/\"1\" --input - /repos/o/r/issues"
    );
    assert_eq!(lines.next().unwrap(), r#"{"title":"T"}"#);
}

#[test]
fn a_gh_that_is_signed_out_or_offline_or_missing_is_a_failure() {
    let (_d, cli) = stand_in("echo 'gh auth login' >&2; exit 4");
    assert_eq!(cli.send(&Call::get("/user")), Err(Failure::NotSignedIn));
    let (_d, cli) = stand_in("echo 'dial tcp: no such host' >&2; exit 1");
    assert_eq!(cli.send(&Call::get("/user")), Err(Failure::Offline));
    let cli = GhCli::default().with_program("/nonexistent/gh");
    assert_eq!(cli.send(&Call::get("/user")), Err(Failure::ToolMissing));
}

#[test]
fn an_error_status_with_a_nonzero_exit_is_still_a_reply() {
    let (_d, cli) = stand_in(
        "printf 'HTTP/2.0 403 Forbidden\\nRetry-After: 7\\n\\n{\"message\":\"rate limit\"}'; exit 1",
    );
    let reply = cli.send(&Call::get("/user")).unwrap();
    assert_eq!(
        (reply.status, reply.header("retry-after")),
        (403, Some("7"))
    );
}
