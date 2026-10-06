use super::super::OpenRouterUsage;

#[test]
fn a_key_with_a_limit_is_one_window_of_credit() {
    let reading = OpenRouterUsage::parse(r#"{"data":{"label":"k","usage":4.2,"limit":20.0,"limit_remaining":15.8}}"#).unwrap();
    assert_eq!((reading.windows[0].label.as_str(), reading.windows[0].used), ("credit", 0.21));
    assert_eq!(reading.note.as_deref(), Some("$4.20 of $20.00 credit"));
}

#[test]
fn a_key_with_no_limit_is_only_the_spend() {
    let reading = OpenRouterUsage::parse(r#"{"data":{"usage":12.5,"limit":null}}"#).unwrap();
    assert!(reading.windows.is_empty());
    assert_eq!(reading.note.as_deref(), Some("$12.50 spent, no limit on the key"));
}

#[test]
fn an_answer_that_is_not_a_key_is_a_reason() {
    assert!(OpenRouterUsage::parse("{}").is_err());
    assert!(OpenRouterUsage::parse("nope").is_err());
}

/// A key with a limit that resets spends from the start of its window: `usage` is the lifetime spend, `limit_remaining` the
/// credit left, as the endpoint answered for a monthly limit.
#[test]
fn a_limit_that_resets_is_read_from_the_credit_left() {
    let answer = r#"{"data":{"usage":7667.82,"limit":1500,"limit_remaining":1351.75,"limit_reset":"monthly"}}"#;
    let reading = OpenRouterUsage::parse(answer).unwrap();
    assert!((reading.windows[0].used - 0.0988).abs() < 0.001, "{:?}", reading.windows);
    assert_eq!(reading.note.as_deref(), Some("$148.25 of $1500.00 credit"));
}
