use super::super::ClaudeUsage;

const NOW: i64 = 1_791_000_000;

/// The shape the endpoint answered with, cut down: unused windows come as null.
const ANSWER: &str = r#"{"five_hour":{"utilization":100.0,"resets_at":"2026-10-03T23:40:00.503834+00:00"},
"seven_day":{"utilization":77.0,"resets_at":"2026-10-05T12:00:00.503857+00:00"},"seven_day_opus":null,"seven_day_sonnet":null,
"extra_usage":{"is_enabled":true,"monthly_limit":10000,"used_credits":10003.0,"currency":"USD","decimal_places":2}}"#;

fn credentials(expires_ms: i64) -> String {
    format!(r#"{{"claudeAiOauth":{{"accessToken":"sk-ant-oat01-x","refreshToken":"r","expiresAt":{expires_ms}}}}}"#)
}

#[test]
fn the_five_hour_and_seven_day_windows_are_read_and_the_empty_ones_left_out() {
    let reading = ClaudeUsage::parse(ANSWER, NOW).unwrap();
    let windows: Vec<_> = reading.windows.iter().map(|w| (w.label.as_str(), w.used)).collect();
    assert_eq!(windows, [("5h", 1.0), ("7d", 0.77)]);
}

#[test]
fn a_reset_is_the_seconds_left_and_never_below_nothing() {
    let at = chrono::DateTime::parse_from_rfc3339("2026-10-03T23:40:00+00:00").unwrap().timestamp();
    let reading = ClaudeUsage::parse(ANSWER, at - 600).unwrap();
    assert_eq!(reading.windows[0].resets_in, Some(600));
    let past = ClaudeUsage::parse(ANSWER, at + 600).unwrap();
    assert_eq!(past.windows[0].resets_in, Some(0));
}

#[test]
fn spend_beyond_the_plan_is_a_note_in_the_currency() {
    let reading = ClaudeUsage::parse(ANSWER, NOW).unwrap();
    assert_eq!(reading.note.as_deref(), Some("Extra usage: $100.03 of $100.00"));
    let off = ANSWER.replace(r#""is_enabled":true"#, r#""is_enabled":false"#);
    assert_eq!(ClaudeUsage::parse(&off, NOW).unwrap().note, None);
}

#[test]
fn an_answer_with_no_windows_or_no_json_is_a_reason_not_an_empty_bar() {
    assert_eq!(ClaudeUsage::parse("{}", NOW), Err("Claude reported no limits".into()));
    assert!(ClaudeUsage::parse("<html>", NOW).is_err());
}

#[test]
fn a_utilization_beyond_the_range_is_held_in_it() {
    let over = r#"{"five_hour":{"utilization":140.0,"resets_at":null}}"#;
    let reading = ClaudeUsage::parse(over, NOW).unwrap();
    assert_eq!((reading.windows[0].used, reading.windows[0].resets_in), (1.0, None));
}

#[test]
fn the_token_comes_from_the_sign_in_unless_it_ran_out() {
    let valid = NOW * 1000 + 60_000;
    assert_eq!(ClaudeUsage::token(&credentials(valid), NOW).as_deref(), Ok("sk-ant-oat01-x"));
    let error = ClaudeUsage::token(&credentials(NOW * 1000 - 1), NOW).unwrap_err();
    assert!(error.contains("has run out"), "{error}");
}

#[test]
fn no_sign_in_says_so_without_echoing_what_was_read() {
    for text in ["", "not json", "{}", r#"{"claudeAiOauth":{"accessToken":""}}"#] {
        assert_eq!(ClaudeUsage::token(text, NOW), Err("Claude is not signed in on this machine".into()), "{text:?}");
    }
}
