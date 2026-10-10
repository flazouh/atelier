use crate::structs::BotId;

#[test]
fn a_short_lower_case_id_is_good() {
    assert_eq!(
        "deep-debugger2".parse::<BotId>().unwrap().as_str(),
        "deep-debugger2"
    );
}

#[test]
fn an_id_with_a_capital_a_space_a_slash_or_a_hyphen_at_the_end_is_refused() {
    for bad in [
        "Bolt",
        "my bot",
        "a/b",
        "-bolt",
        "bolt-",
        "",
        "../x",
        &"a".repeat(33),
    ] {
        assert!(bad.parse::<BotId>().is_err(), "{bad:?}");
    }
}

#[test]
fn an_id_in_a_file_is_checked_when_it_is_read() {
    assert!(serde_json::from_str::<BotId>("\"../etc\"").is_err());
    assert_eq!(
        serde_json::from_str::<BotId>("\"dot\"").unwrap().as_str(),
        "dot"
    );
}
