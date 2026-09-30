use super::*;
/// The prompt holds what the reader asked and the start of what the agent answered.
#[test]
fn the_prompt_holds_the_first_exchange() {
    let prompt = prompt("fix the flaky lease test", &"The lease was never released. ".repeat(200));
    assert!(prompt.contains("fix the flaky lease test"));
    assert!(prompt.contains("The lease was never released."));
    assert!(prompt.len() < 3000, "the answer is cut short: {}", prompt.len());
}
/// A draft becomes a title: its first line, with no quotes, heading mark, "Title:" or full stop, and
/// short; an empty draft is no title.
#[test]
fn a_draft_becomes_a_short_title() {
    assert_eq!(clean("\"Fix the flaky lease test.\"\n\nMore words."), Some("Fix the flaky lease test".into()));
    assert_eq!(clean("# Title: Keep TWO"), Some("Keep TWO".into()));
    assert_eq!(clean("   \n  "), None);
    let long = clean(&"word ".repeat(40)).unwrap();
    assert!(long.chars().count() <= TITLE_MOST && !long.ends_with(' '), "{long:?}");
}
