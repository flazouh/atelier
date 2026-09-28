//! Prints everything a language server says about one file, so a wiring problem can be seen rather
//! than guessed at.
//!
//! Usage: cargo run -p lathe-lsp --example probe -- <root> <file> [seconds]

use std::{path::PathBuf, time::Duration};

use lathe_lsp::{LspClient, ServerMessage};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("a workspace root"));
    let file = PathBuf::from(args.next().expect("a file inside it"));
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let text = std::fs::read_to_string(&file).expect("the file reads");

    let (mut client, init) = LspClient::spawn(std::path::Path::new("rust-analyzer"), &[], &root, None, Duration::from_secs(60))
        .expect("rust-analyzer starts");
    println!("server: {:?}", init.server_info.map(|i| i.name));
    client.did_open(&file, "rust", 1, &text).expect("the file opens");
    println!("opened {} at version 1", file.display());

    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    let mut version = 1;
    let mut nudged = false;
    while std::time::Instant::now() < deadline {
        // Halfway through, send the same text as a new version, as the editor does on Check.
        if !nudged && std::time::Instant::now() > deadline - Duration::from_secs(seconds / 2) {
            version += 1;
            client.did_change(&file, version, &text).expect("the change is sent");
            println!("--- sent version {version}");
            nudged = true;
        }
        match client.messages.recv_timeout(Duration::from_millis(500)) {
            Ok(ServerMessage::Diagnostics(params)) => println!(
                "diagnostics: version={:?} count={} {:?}",
                params.version,
                params.diagnostics.len(),
                params.diagnostics.iter().map(|d| d.message.as_str()).collect::<Vec<_>>()
            ),
            Ok(ServerMessage::Log(text)) => println!("log: {text}"),
            Ok(ServerMessage::Status { quiescent }) => println!("status: quiescent={quiescent}"),
            Ok(ServerMessage::Exited) => {
                println!("the server exited");
                break;
            }
            Err(_) => {}
        }
    }
    client.shutdown(Duration::from_secs(10)).ok();
}
