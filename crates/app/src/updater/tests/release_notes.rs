use super::super::{NoteLine, helpers::written_date, is_older, release_date, release_notes};

fn line(lead: &str, text: &str) -> NoteLine {
    NoteLine { lead: lead.into(), text: text.into() }
}

#[test]
fn a_bullet_with_a_bold_lead_gives_the_lead_and_the_text() {
    let notes = release_notes("## What is new in 0.1.2\n\n- **Files tree:** a menu on each row.\n- **Fixes.** A row fills the width.\n");
    assert_eq!(notes, vec![line("Files tree", "A menu on each row."), line("Fixes.", "A row fills the width.")], "the text starts with a capital");
}

#[test]
fn the_headings_and_the_blank_lines_are_not_notes() {
    assert_eq!(release_notes("# Title\n\n## Part\n\n- **A:** b"), vec![line("A", "B")]);
}

#[test]
fn the_older_kind_headings_are_ignored_and_every_bullet_stays() {
    let notes = release_notes("## What is new in 1\n\n### New\n\n- **A:** a\n\n### Fixed\n- **B:** b");
    assert_eq!(notes, vec![line("A", "A"), line("B", "B")]);
}

#[test]
fn the_released_line_is_not_a_note() {
    assert_eq!(release_notes("## What is new in 1\nReleased: 2026-10-09\n- **A:** a"), vec![line("A", "A")]);
    assert!(release_notes("## What is new in 1\nReleased: 2026-10-09\n").is_empty(), "a date alone is no note");
}

#[test]
fn a_bullet_with_no_lead_is_all_lead() {
    assert_eq!(release_notes("- Something changed"), vec![line("Something changed", "")]);
}

#[test]
fn words_with_no_bullet_are_one_note_and_nothing_is_none() {
    assert_eq!(release_notes("It is faster.\nAnd smaller."), vec![line("What changed", "It is faster. And smaller.")]);
    assert!(release_notes("").is_empty());
    assert!(release_notes("## Only a heading\n").is_empty());
}

#[test]
fn the_released_line_gives_the_date_as_the_sheet_writes_it() {
    assert_eq!(release_date("## What is new in 1\nReleased: 2026-10-09\n- **A:** a").as_deref(), Some("Oct 9, 2026"));
    assert_eq!(release_date("## What is new in 1\nReleased:2026-01-31").as_deref(), Some("Jan 31, 2026"), "a space after the colon is optional");
    assert_eq!(release_date("## What is new in 1\n- **A:** a"), None, "no line, no date");
    assert_eq!(release_date("Released: soon"), None, "words are no date");
}

#[test]
fn a_date_is_written_with_an_english_month_and_no_zero_on_the_day() {
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    for (at, name) in months.iter().enumerate() {
        assert_eq!(written_date(&format!("2027-{:02}-05", at + 1)), Some(format!("{name} 5, 2027")));
    }
    assert_eq!(written_date("2026-12-31").as_deref(), Some("Dec 31, 2026"));
}

#[test]
fn a_day_that_the_calendar_does_not_have_is_no_date() {
    assert_eq!(written_date("2026-02-29"), None, "2026 is not a leap year");
    assert_eq!(written_date("2028-02-29").as_deref(), Some("Feb 29, 2028"));
    assert_eq!(written_date("1900-02-29"), None, "a century is not a leap year");
    assert_eq!(written_date("2000-02-29").as_deref(), Some("Feb 29, 2000"));
    for bad in ["2026-04-31", "2026-13-01", "2026-00-10", "2026-10-00", "2026-10-32", "26-10-09", "2026-1-9", "2026-10", "2026-10-09-01", "", "x-y-z"] {
        assert_eq!(written_date(bad), None, "{bad:?}");
    }
}

#[test]
fn versions_compare_by_number() {
    assert!(is_older("0.1.8", "0.1.9"));
    assert!(is_older("0.1.9", "0.1.10"), "numbers, not letters");
    assert!(!is_older("0.1.9", "0.1.9"));
    assert!(!is_older("0.2.0", "0.1.9"));
    assert!(!is_older("dev", "0.1.9") && !is_older("0.1.9", "dev"), "a word is no version");
}
