use super::super::{NoteLine, release_notes};
use atelier_ui::ReleaseKind::{self, Added, Changed, Design, Faster, Fixed, Improved};

fn line(lead: &str, text: &str) -> NoteLine {
    NoteLine { kind: Improved, lead: lead.into(), text: text.into() }
}

fn typed(kind: ReleaseKind, lead: &str, text: &str) -> NoteLine {
    NoteLine { kind, ..line(lead, text) }
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
fn a_bullet_takes_its_kind_from_the_heading_above_it() {
    let notes = release_notes("## What is new in 1\n\n### New\n\n- **A:** a\n- **B:** b\n\n### Fixed\n\n- **C:** c\n\n### Improved\n- **D:** d");
    assert_eq!(notes, vec![typed(Added, "A", "A"), typed(Added, "B", "B"), typed(Fixed, "C", "C"), typed(Improved, "D", "D")]);
    assert_eq!(release_notes("- **A:** a"), vec![typed(Improved, "A", "A")], "with no heading: improved");
    assert_eq!(release_notes("### Fixes\n- **A:** a")[0].kind, Fixed, "the word decides: fix");
}

#[test]
fn every_heading_word_names_one_of_the_six_kinds() {
    let kind = |heading: &str| release_notes(&format!("### {heading}\n- **A:** a"))[0].kind;
    assert_eq!(kind("New"), Added);
    assert_eq!(kind("Added"), Added);
    assert_eq!(kind("Improved"), Improved);
    assert_eq!(kind("Faster"), Faster);
    assert_eq!(kind("Performance"), Faster);
    assert_eq!(kind("Fixed"), Fixed);
    assert_eq!(kind("Bug fixes"), Fixed);
    assert_eq!(kind("Changed"), Changed);
    assert_eq!(kind("Design"), Design);
    assert_eq!(kind("Something else"), Improved, "an unknown heading is improved");
}
