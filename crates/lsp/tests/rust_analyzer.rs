//! Talks to a real `rust-analyzer` over stdio. This is the only proof that the client works: the unit
//! tests cover the framing and the routing, but not a live server.
//!
//! The test skips itself when `rust-analyzer` is not installed, and says so. Set `LATHE_REQUIRE_LSP=1`
//! where the server is installed, and a missing server fails the run rather than passing quietly.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use lathe_lsp::LspClient;
use lsp_types::{DiagnosticSeverity, GotoDefinitionResponse, Position};

/// rust-analyzer indexes a crate before it answers, so the waits are generous.
const READY: Duration = Duration::from_secs(120);
const ASK: Duration = Duration::from_secs(60);

fn rust_analyzer() -> Option<PathBuf> {
    let direct = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cargo/bin/rust-analyzer");
    if Command::new(&direct).arg("--version").output().is_ok_and(|o| o.status.success()) {
        return Some(direct);
    }
    Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| PathBuf::from("rust-analyzer"))
}

/// A tiny crate on disk: one function with a type error, and one call to check a definition against.
fn fixture(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir.join("src")).expect("the temp dir is writable");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"lathe-lsp-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    )
    .expect("the manifest is written");
    let source = "pub fn width() -> u32 {\n    7\n}\n\npub fn broken() -> u32 {\n    let text: u32 = \"not a number\";\n    text + width()\n}\n";
    let path = dir.join("src/lib.rs");
    fs::write(&path, source).expect("the source is written");
    path
}

#[test]
fn rust_analyzer_reports_a_real_type_error_and_finds_a_definition() {
    let Some(program) = rust_analyzer() else {
        // A skip must never read as a pass. Set LATHE_REQUIRE_LSP=1 where the server is installed, and
        // a missing server fails the run instead of quietly passing it.
        assert!(
            std::env::var_os("LATHE_REQUIRE_LSP").is_none(),
            "LATHE_REQUIRE_LSP is set but rust-analyzer is not installed, so the live client is unproven"
        );
        eprintln!("skipped: rust-analyzer is not installed, so the live client is unproven here");
        return;
    };
    let dir = std::env::temp_dir().join(format!("lathe-lsp-{}", std::process::id()));
    let file = fixture(&dir);
    let text = fs::read_to_string(&file).expect("the fixture reads back");

    let (mut client, init) = LspClient::spawn(program.to_str().expect("a UTF-8 path"), &[], &dir, READY)
        .expect("rust-analyzer starts and shakes hands");
    assert!(
        init.capabilities.definition_provider.is_some(),
        "a server that cannot answer definition is no use here"
    );

    client.did_open(&file, "rust", 1, &text).expect("the file opens");

    // The type error: `let text: u32 = "not a number";`
    let mut error = None;
    let deadline = std::time::Instant::now() + READY;
    while std::time::Instant::now() < deadline && error.is_none() {
        let Ok(params) = client.wait_for_diagnostics(&file, ASK) else { break };
        error = params
            .diagnostics
            .into_iter()
            .find(|d| d.severity == Some(DiagnosticSeverity::ERROR));
    }
    let error = error.expect("rust-analyzer reports the type error in the fixture");
    assert_eq!(error.range.start.line, 5, "the error sits on the `let text: u32` line");
    assert!(
        error.message.to_lowercase().contains("mismatch") || error.message.contains("u32"),
        "the message names the type problem: {}",
        error.message
    );

    // `width()` on line 6 is defined on line 0.
    let column = text.lines().nth(6).expect("line 6 exists").find("width").expect("the call is there") as u32;
    let answer = client
        .definition(&file, Position { line: 6, character: column + 1 }, ASK)
        .expect("the server answers the definition request")
        .expect("it knows where width is defined");
    let target = match answer {
        GotoDefinitionResponse::Scalar(location) => location.range.start.line,
        GotoDefinitionResponse::Array(locations) => {
            locations.first().expect("at least one location").range.start.line
        }
        GotoDefinitionResponse::Link(links) => links.first().expect("at least one link").target_range.start.line,
    };
    assert_eq!(target, 0, "width is defined on the first line");

    client.shutdown(ASK).expect("the server stops");
    fs::remove_dir_all(&dir).ok();
}
