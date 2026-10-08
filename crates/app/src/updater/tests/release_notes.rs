use super::super::{NoteLine, release_notes};

fn line(lead: &str, text: &str) -> NoteLine {
    NoteLine { lead: lead.into(), text: text.into() }
}

#[test]
fn a_bullet_with_a_bold_lead_gives_the_lead_and_the_text() {
    let notes = release_notes("## What is new in 0.1.2\n\n- **Files tree:** a menu on each row.\n- **Fixes.** A row fills the width.\n");
    assert_eq!(notes, vec![line("Files tree", "a menu on each row."), line("Fixes.", "A row fills the width.")]);
}

#[test]
fn the_headings_and_the_blank_lines_are_not_notes() {
    assert_eq!(release_notes("# Title\n\n## Part\n\n- **A:** b"), vec![line("A", "b")]);
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
