use atelier_capabilities::CapError;

use super::fake::display;
use crate::{
    CliRunner, RunFailure, Runner,
    helpers::{map_failure, parse_display_date, remote_command, shell_quote},
};

/// 2026-09-18 10:39 UTC.
const AT: i64 = 1_789_727_940_000;

#[test]
fn gmail_dates_are_read_in_english_and_french_at_the_minute() {
    for text in [
        "Fri, 18 Sept 2026, 10:39",
        "Fri, Sep 18, 2026, 10:39",
        "Fri, Sep 18, 2026, 10:39 AM",
        "ven. 18 sept. 2026, 10:39",
        "18 September 2026 10:39",
    ] {
        assert_eq!(parse_display_date(text), Some(AT), "{text}");
    }
    assert_eq!(
        parse_display_date("Fri, Sep 18, 2026, 10:39 PM"),
        Some(1_789_771_140_000)
    );
    assert_eq!(
        parse_display_date("Sat, 19 Sept 2026, 12:05 AM"),
        Some(1_789_776_300_000)
    );
    // The French weekday `mar.` (Tuesday) is not read as the month March.
    assert_eq!(
        parse_display_date("mar. 22 sept. 2026, 08:00"),
        Some(1_790_064_000_000)
    );
    assert_eq!(parse_display_date("1 Jan 1970, 00:00"), Some(0));
    assert_eq!(parse_display_date("29 Feb 2024"), Some(1_709_164_800_000));
}

#[test]
fn a_date_that_cannot_be_read_is_none() {
    for text in [
        "",
        "Today",
        "18:05",
        "Sep 2026",
        "32 Sep 2026",
        "18 Foo 2026",
        "18 Sep 1969",
    ] {
        assert_eq!(parse_display_date(text), None, "{text:?}");
    }
}

#[test]
fn the_fake_prints_dates_that_read_back() {
    for ms in [0, 1_760_000_000_000, AT, 1_709_164_800_000 + 86_399_000] {
        assert_eq!(
            parse_display_date(&display(ms)),
            Some(ms - ms % 60_000),
            "{ms}"
        );
    }
}

#[test]
fn words_are_quoted_for_the_remote_shell() {
    assert_eq!(shell_quote("search"), "search");
    assert_eq!(shell_quote("from:bob@x.com"), "from:bob@x.com");
    assert_eq!(shell_quote("a b"), "'a b'");
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
    assert_eq!(shell_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
    assert_eq!(shell_quote("a;b"), "'a;b'");
    assert_eq!(
        remote_command(
            "/Users/alex/.local/bin/gmailcli",
            &["search".into(), "x y".into(), "-json".into()]
        ),
        "/Users/alex/.local/bin/gmailcli search 'x y' -json"
    );
}

#[test]
fn what_the_tool_prints_maps_to_the_errors_of_the_capability() {
    let exit = |text: &str| RunFailure::Exit {
        code: Some(1),
        stdout: text.into(),
        stderr: String::new(),
    };
    let rate = CapError::RateLimited {
        retry_after_ms: 60_000,
    };
    let table = [
        (
            "error: the browser is not signed in to Gmail. Open mail.google.com",
            CapError::NotSignedIn,
        ),
        (
            "error: the browser call failed: redirected to accounts.google.com",
            CapError::NotSignedIn,
        ),
        (
            "error: the browser call failed: Rate limit exceeded",
            rate.clone(),
        ),
        ("error: Gmail says: unusual traffic from your network", rate),
        (
            "ssh: connect to host mac port 22: Connection refused",
            CapError::Offline,
        ),
        (
            "ssh: connect to host mac port 22: No route to host",
            CapError::Offline,
        ),
        ("error: net::ERR_INTERNET_DISCONNECTED", CapError::Offline),
    ];
    for (text, want) in table {
        assert_eq!(map_failure(&exit(text)), want, "{text}");
    }
    let code = |f: RunFailure| match map_failure(&f) {
        CapError::Provider { code, message } => (code, message),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        code(exit(
            "error: ego-browser is not installed. See the control-browser skill"
        ))
        .0,
        "not_installed"
    );
    assert_eq!(
        code(RunFailure::Spawn("gmailcli: No such file".into())).0,
        "not_installed"
    );
    let (c, m) = code(exit("error: the browser returned nothing at all\nhelp: x"));
    assert_eq!(
        (c.as_str(), m.as_str()),
        ("gmailcli", "the browser returned nothing at all")
    );
    assert_eq!(code(exit("")).1, "gmailcli failed with no message");
    assert_eq!(
        code(exit(&format!("error: {}", "x".repeat(900)))).1.len(),
        300
    );
}

#[test]
fn the_cli_runner_runs_a_program_here() {
    let echo = CliRunner::local("echo");
    assert_eq!(echo.run(&["a".into(), "b".into()]).unwrap(), "a b\n");
    match CliRunner::local("false").run(&[]) {
        Err(RunFailure::Exit { code, .. }) => assert_eq!(code, Some(1)),
        other => panic!("{other:?}"),
    }
    let missing = CliRunner::local("no-such-program-gmailcli-test").run(&[]);
    assert!(matches!(missing, Err(RunFailure::Spawn(_))), "{missing:?}");
    assert!(matches!(
        map_failure(&missing.unwrap_err()),
        CapError::Provider { code, .. } if code == "not_installed"
    ));
}

#[test]
fn the_cli_runner_reads_and_removes_only_what_the_provider_wrote() {
    let runner = CliRunner::local("echo");
    assert!(runner.reads_files());
    let tmp = std::path::PathBuf::from(runner.temp_dir());
    let base = tmp.join(format!("atelier-gmail-test-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("a.txt"), b"hello").unwrap();
    assert_eq!(
        runner
            .read_file(base.join("a.txt").to_str().unwrap())
            .unwrap(),
        b"hello"
    );
    runner.remove_dir(base.to_str().unwrap());
    assert!(
        !base.exists(),
        "a directory with the provider's name is removed"
    );

    let other = tmp.join(format!("keep-me-{}", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    runner.remove_dir(other.to_str().unwrap());
    assert!(other.exists(), "any other directory is left alone");
    std::fs::remove_dir_all(&other).unwrap();
    assert!(matches!(
        runner.read_file("/no/such/file"),
        Err(RunFailure::Exit { .. })
    ));
}
