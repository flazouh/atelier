use super::super::CodexUsage;

const NOW: i64 = 1_791_000_000;

/// What `codex app-server` wrote for the two requests, with the notices it adds between them.
fn output(limits: &str) -> String {
    format!(
        "{}\n{}\n{}\n",
        r#"{"id":1,"result":{"userAgent":"atelier/0.160.0","codexHome":"/home/user/.codex"}}"#,
        r#"{"method":"account/updated","params":{"authMode":"chatgpt","planType":"free"}}"#,
        format_args!(r#"{{"id":2,"result":{{"rateLimits":{limits}}}}}"#),
    )
}

#[test]
fn a_window_is_labelled_by_its_length_in_the_biggest_unit_it_fills() {
    for (minutes, label) in [(300, "5h"), (10_080, "7d"), (43_200, "30d"), (45, "45m"), (90, "90m")] {
        let limits = format!(r#"{{"primary":{{"usedPercent":10,"windowDurationMins":{minutes},"resetsAt":{}}},"secondary":null}}"#, NOW + 60);
        let reading = CodexUsage::parse(&output(&limits), NOW).unwrap();
        assert_eq!(reading.windows[0].label, label, "{minutes} minutes");
    }
}

#[test]
fn the_primary_and_secondary_windows_are_both_read_with_their_resets() {
    let limits = format!(
        r#"{{"primary":{{"usedPercent":25,"windowDurationMins":300,"resetsAt":{}}},"secondary":{{"usedPercent":60,"windowDurationMins":10080,"resetsAt":{}}},"planType":"plus"}}"#,
        NOW + 1800,
        NOW + 86_400
    );
    let reading = CodexUsage::parse(&output(&limits), NOW).unwrap();
    let windows: Vec<_> = reading.windows.iter().map(|w| (w.label.as_str(), w.used, w.resets_in)).collect();
    assert_eq!(windows, [("5h", 0.25, Some(1800)), ("7d", 0.6, Some(86_400))]);
    assert_eq!(reading.note.as_deref(), Some("Codex plus plan"));
}

#[test]
fn a_server_that_said_nothing_is_not_installed_or_not_signed_in() {
    assert!(CodexUsage::parse("", NOW).unwrap_err().contains("did not answer"));
    assert!(CodexUsage::parse(r#"{"id":1,"result":{}}"#, NOW).unwrap_err().contains("did not answer"));
}

#[test]
fn a_refusal_is_told_in_the_servers_words() {
    let refused = r#"{"id":2,"error":{"code":-32603,"message":"Codex is not signed in"}}"#;
    assert_eq!(CodexUsage::parse(refused, NOW), Err("Codex is not signed in".into()));
}

#[test]
fn no_windows_is_a_reason_not_an_empty_bar() {
    let none = output(r#"{"primary":null,"secondary":null}"#);
    assert_eq!(CodexUsage::parse(&none, NOW), Err("Codex reported no limits".into()));
}
