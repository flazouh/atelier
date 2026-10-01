use atelier_agents::session::{Attachment, message_text};

use super::Comments;
use crate::merged::{Anchor, Merged, Side};

fn anchor(side: Side, first: u32, last: u32, quote: &str) -> Anchor {
    Anchor { side, first_line: first, last_line: last, quote: quote.into() }
}

#[test]
fn comments_are_added_removed_and_edited_by_id() {
    let mut comments = Comments::new();
    let a = comments.add(0, "a.rs", anchor(Side::Current, 2, 3, "x\ny"), "why?");
    let b = comments.add(0, "b.rs", anchor(Side::Current, 1, 1, "z"), "rename");
    assert_ne!(a, b);
    assert_eq!(comments.all().len(), 2);
    assert!(comments.set_body(a, "why this?"));
    assert_eq!(comments.for_file("a.rs").next().unwrap().body, "why this?");
    assert!(comments.remove(b) && !comments.remove(b));
    assert!(!comments.set_body(b, "x"));
    assert_eq!(comments.for_file("b.rs").count(), 0);
}

#[test]
fn a_comment_follows_its_quoted_rows_when_the_file_moves_on() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Current, 3, 4, "let x = 1;\nlet y = 2;"), "combine these");
    comments.reanchor("a.rs", "// new header\n// second\nfn f() {\n    x\n}\nlet x = 1;\nlet y = 2;\n");
    let c = &comments.all()[0];
    assert_eq!((c.first_line, c.last_line, c.stale), (6, 7, false));
}

#[test]
fn a_comment_picks_the_match_nearest_to_where_it_was() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Current, 5, 5, "dup"), "here");
    comments.reanchor("a.rs", "dup\nx\nx\nx\ndup\nx\nx\nx\nx\ndup\n");
    assert_eq!(comments.all()[0].first_line, 5);
}

#[test]
fn a_comment_whose_rows_are_gone_is_stale_and_keeps_its_old_lines() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Current, 2, 2, "gone row"), "hm");
    comments.reanchor("a.rs", "a\nb\nc\n");
    let c = &comments.all()[0];
    assert_eq!((c.first_line, c.stale), (2, true));
    comments.reanchor("a.rs", "a\ngone row\nc\n");
    assert!(!comments.all()[0].stale, "found again");
}

#[test]
fn only_comments_on_the_changed_file_and_the_current_side_move() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Removed, 4, 4, "old row"), "on removed");
    comments.add(0, "b.rs", anchor(Side::Current, 1, 1, "row"), "other file");
    comments.reanchor("a.rs", "no such row\n");
    assert!(!comments.all()[0].stale && comments.all()[0].first_line == 4);
    assert!(!comments.all()[1].stale);
}

#[test]
fn a_comment_on_a_multi_line_quote_needs_all_its_rows_in_a_row() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Current, 1, 2, "a\nb"), "x");
    comments.reanchor("a.rs", "a\nc\nb\n");
    assert!(comments.all()[0].stale);
}

#[test]
fn comments_become_the_attachments_of_the_next_message_and_are_then_gone() {
    let merged = Merged::diff("1\n2\n3\n", "1\nX\n3\n");
    let mut comments = Comments::new();
    comments.add(0, "a.rs", merged.anchor(2..3).unwrap(), "use a better name");
    comments.add(0, "a.rs", merged.anchor(1..2).unwrap(), "why remove this?");
    let attachments = comments.take_attachments();
    assert!(comments.all().is_empty());
    assert_eq!(
        attachments,
        vec![
            Attachment::LineComment { path: "a.rs".into(), first_line: 2, last_line: 2, removed: false, quote: "X".into(), body: "use a better name".into() },
            Attachment::LineComment { path: "a.rs".into(), first_line: 2, last_line: 2, removed: true, quote: "2".into(), body: "why remove this?".into() },
        ]
    );
    let text = message_text("Please fix these", &attachments);
    assert!(text.starts_with("Please fix these\n\nReview comment on a.rs, line 2:\n> X\nuse a better name"));
    assert!(text.contains("(lines the agent removed, numbered as they were before)"));
}

#[test]
fn attachments_leave_the_comments_in_place_until_they_are_taken() {
    let mut comments = Comments::new();
    comments.add(0, "a.rs", anchor(Side::Current, 1, 1, "x"), "c");
    assert_eq!(comments.attachments().len(), 1);
    assert_eq!(comments.all().len(), 1);
}
