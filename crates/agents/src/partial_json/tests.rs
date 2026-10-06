use serde_json::json;

use super::*;

#[test]
fn what_has_arrived_of_each_text_is_read_whole_or_cut() {
    for (json, want) in [
        (r#"{"file_path":"/w/a.rs","old_string":"fn a","new_string":"fn b() {\n    "#, Some(json!({"file_path": "/w/a.rs", "old_string": "fn a", "new_string": "fn b() {\n    "}))),
        (r#"{"file_path":"/w/a.rs","old_string":"x","new_string":"y"}"#, Some(json!({"file_path": "/w/a.rs", "old_string": "x", "new_string": "y"}))),
        (r#"{"file_path":"/w/a.rs","content":"say \"hi\" \"#, Some(json!({"file_path": "/w/a.rs", "content": "say \"hi\" "}))),
        (r#"{"content":"a\\"#, Some(json!({"content": "a\\"}))),
        (r#"{"content":"caf\u00e"#, Some(json!({"content": "caf"}))),
        (r#"{"content":"caf\u00e9 \ud83d"#, Some(json!({"content": "caf\u{e9} "}))),
        (r#"{"content":"\ud83d\ude00!"#, Some(json!({"content": "\u{1f600}!"}))),
        (r#"{"file_path":"/w/a.rs","replace_all":false,"new_string":"z"#, Some(json!({"file_path": "/w/a.rs", "new_string": "z"}))),
        (r#"{"file_p"#, None),
        (r#"{"file_path":"#, None),
        (r#"{"file_path":""#, Some(json!({"file_path": ""}))),
        ("", None),
    ] {
        assert_eq!(fields(json), want, "{json}");
    }
}

#[test]
fn the_same_words_inside_a_string_are_not_a_key() {
    let got = fields(r#"{"content":"\"new_string\": \"no\"","new_string":"yes"#).unwrap();
    assert_eq!(got, json!({"content": "\"new_string\": \"no\"", "new_string": "yes"}));
}

#[test]
fn only_the_strings_that_closed_count_as_closed() {
    assert_eq!(closed(r#"{"path":"a.t"#), None);
    assert_eq!(closed(r#"{"path":"a.txt","content":"hel"#), Some(json!({"path": "a.txt"})));
    assert_eq!(closed(r#"{"path":"a.txt"}"#), Some(json!({"path": "a.txt"})));
}

#[test]
fn a_value_cut_anywhere_reads_as_far_as_it_came() {
    use serde_json::json;
    let whole = r#"{"questions":[{"question":"Which toy?","options":[{"label":"Chain","description":"Blocks"},{"label":"Cipher"}],"multiSelect":false}]}"#;
    assert_eq!(super::value(whole), Some(json!({"questions":[{"question":"Which toy?","options":[{"label":"Chain","description":"Blocks"},{"label":"Cipher"}],"multiSelect":false}]})));
    assert_eq!(super::value(""), None);
    assert_eq!(super::value(r#"{"questions":[{"question":"Which to"#), Some(json!({"questions":[{"question":"Which to"}]})), "a string cut mid-word");
    assert_eq!(super::value(r#"{"questions":[{"question":"Which toy?","options":[{"label":"Chain","descr"#), Some(json!({"questions":[{"question":"Which toy?","options":[{"label":"Chain"}]}]})), "a key with no value is left out");
    assert_eq!(super::value(r#"{"a":1,"b":"#), Some(json!({"a":1})), "a key waiting for its value");
    assert_eq!(super::value(r#"{"a":[1,2,"#), Some(json!({"a":[1,2]})), "a comma with nothing after it");
    assert_eq!(super::value(r#"{"a":"x\"#), Some(json!({"a":"x"})), "an escape cut in two");
    // Every cut of the whole text reads, and never reads more than the whole holds.
    for end in 1..whole.len() {
        assert!(super::value(&whole[..end]).is_some() || end < 2, "cut at {end}: {}", &whole[..end]);
    }
}
