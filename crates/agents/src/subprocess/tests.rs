use super::*;

#[test]
fn colour_codes_are_not_words() {
    assert_eq!(strip_ansi("Run \u{1b}[36msecurity find-identity\u{1b}[0m to see them"), "Run security find-identity to see them");
    assert_eq!(strip_ansi("\u{1b}[1;31merror:\u{1b}[0m the key is missing"), "error: the key is missing");
    assert_eq!(strip_ansi("\u{1b}[2K\u{1b}[1Gready"), "ready", "cursor codes go too");
    assert_eq!(strip_ansi("plain, with é and 日本語"), "plain, with é and 日本語");
    assert_eq!(strip_ansi(""), "");
}

#[test]
fn a_title_or_a_link_goes_to_its_end() {
    assert_eq!(strip_ansi("a\u{1b}]0;title\u{7}b"), "ab");
    assert_eq!(strip_ansi("\u{1b}]8;;https://x.test\u{1b}\\link\u{1b}]8;;\u{1b}\\"), "link");
}

#[test]
fn a_cut_off_escape_takes_nothing_else() {
    assert_eq!(strip_ansi("end\u{1b}"), "end");
    assert_eq!(strip_ansi("end\u{1b}[31"), "end");
    assert_eq!(strip_ansi("a\u{1b}Mb"), "ab", "a two-character escape");
}

#[test]
fn the_tail_a_row_shows_has_no_colour() {
    let stderr = "starting\n\u{1b}[31merror\u{1b}[0m: no identity\nRun \u{1b}[36msecurity\u{1b}[0m\n\n";
    assert_eq!(stderr_tail(stderr), "starting\nerror: no identity\nRun security");
    assert_eq!(exit_why(Some(1), &stderr_tail(stderr)), "the agent exited with code 1: Run security");
}
