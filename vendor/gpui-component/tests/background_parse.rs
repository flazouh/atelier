//! The editor's background parse (PATCHES.md, patch 2): the UI thread only moves the old colours with
//! the edit, a parse on another thread brings the new ones, one parse runs at a time, and a parse for
//! an older text is dropped.
#![cfg(feature = "tree-sitter-languages")]

use gpui_component::highlighter::{HighlightTheme, ParseQueue, SyntaxHighlighter, follow_range};
use ropey::Rope;
use tree_sitter::{InputEdit, Point};

fn point_of(text: &str, offset: usize) -> Point {
    let before = &text[..offset];
    Point::new(before.matches('\n').count(), offset - before.rfind('\n').map_or(0, |n| n + 1))
}

/// Replaces `start..end` of `text` with `insert`, and says so as an edit.
fn replace(text: &mut String, start: usize, end: usize, insert: &str) -> InputEdit {
    let (start_position, old_end_position) = (point_of(text, start), point_of(text, end));
    text.replace_range(start..end, insert);
    InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start + insert.len(),
        start_position,
        old_end_position,
        new_end_position: point_of(text, start + insert.len()),
    }
}

fn edit(start: usize, old_end: usize, new_end: usize) -> InputEdit {
    let at = |b: usize| Point::new(0, b);
    InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: at(start),
        old_end_position: at(old_end),
        new_end_position: at(new_end),
    }
}

/// A styled range at 10..20 after each kind of edit.
#[test]
fn a_styled_range_follows_each_kind_of_edit() {
    let cases = [
        ("three bytes typed before it", edit(4, 4, 7), 13..23),
        ("three bytes typed inside it", edit(14, 14, 17), 10..23),
        ("typed right after it", edit(20, 20, 23), 10..20),
        ("typed right before it", edit(10, 10, 13), 13..23),
        ("three bytes typed well after it", edit(30, 30, 33), 10..20),
        ("a delete across its end", edit(15, 25, 15), 10..15),
        ("a delete across its start", edit(5, 12, 5), 5..13),
        ("a delete of all of it", edit(8, 22, 8), 8..8),
        ("its middle replaced by more", edit(12, 14, 20), 10..26),
    ];
    for (what, edit, want) in cases {
        assert_eq!(follow_range(&(10..20), &edit), want, "{what}");
    }
}

/// Before any parse lands, the old colours stand where their text went.
#[test]
fn the_old_colours_move_with_the_text_until_a_parse_lands() {
    let theme = HighlightTheme::default_dark();
    let mut text = String::from("fn a() { let s = \"quoted\"; }");
    let mut h = SyntaxHighlighter::new("rust");
    assert!(h.update(None, &Rope::from(text.as_str()), None));
    let string_style = |h: &SyntaxHighlighter, text: &str| {
        let at = text.find('"').unwrap();
        let rope = Rope::from(text);
        h.styles(&(0..rope.len()), theme.as_ref()).into_iter().find(|(r, _)| r.contains(&at)).map(|(r, s)| (r, s.color))
    };
    let (before, colour) = string_style(&h, &text).unwrap();
    assert!(colour.is_some(), "the string has a colour");
    let edit = replace(&mut text, 3, 3, "xyz");
    h.edit_tree(Some(edit), &Rope::from(text.as_str()));
    let (after, moved) = string_style(&h, &text).unwrap();
    assert_eq!(after, before.start + 3..before.end + 3, "the string's colour moved with it, with no parse");
    assert_eq!(moved, colour);
}

fn fresh(language: &str, text: &str) -> SyntaxHighlighter {
    let mut h = SyntaxHighlighter::new(language);
    h.set_injection_budget(None);
    assert!(h.update(None, &Rope::from(text), None));
    h
}

#[track_caller]
fn assert_matches_fresh(h: &SyntaxHighlighter, language: &str, text: &str, what: &str) {
    let theme = HighlightTheme::default_dark();
    let rope = Rope::from(text);
    let fresh = fresh(language, text);
    assert_eq!(h.injection_layer_ranges(), fresh.injection_layer_ranges(), "{what}: layers");
    assert_eq!(h.styles(&(0..rope.len()), theme.as_ref()), fresh.styles(&(0..rope.len()), theme.as_ref()), "{what}: styles");
}

/// Edits applied as the editor now applies them, each followed by a background parse: once the parse
/// lands, the colours are a synchronous parse's.
#[test]
fn a_landed_background_parse_equals_a_synchronous_one() {
    let cases = [
        ("rust", "fn a() { let s = format!(\"{}\", 1); }\nfn b() { vec![1, 2]; }\n", &["x", "m!(a)", "\"", "(", ")", "\n"][..]),
        ("markdown", "# `a` *b*\n\n```rust\nfn a() {}\n```\n\n- `c` [l](u)\n", &["```", "`", "*", "\n", "x", "```rust\n"][..]),
        ("html", "<p><script>let a = 1;</script><style>p { color: red }</style></p>\n", &["<script>", "</script>", "x", "\"", ">"][..]),
    ];
    for (language, start, snippets) in cases {
        let mut text = start.to_string();
        let mut h = fresh(language, &text);
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        for step in 0..60 {
            let mut at = next(text.len() + 1);
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            let edit = if next(3) == 0 {
                let mut end = (at + next(8)).min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                replace(&mut text, at, end, "")
            } else {
                replace(&mut text, at, at, snippets[next(snippets.len())])
            };
            h.edit_tree(Some(edit), &Rope::from(text.as_str()));
            let parsed = h.background_parse().unwrap().run().unwrap();
            assert!(h.apply_parsed(parsed), "the parse is for the text held");
            assert_matches_fresh(&h, language, &text, &format!("{language}, edit {step}"));
        }
    }
}

/// 50 keystrokes while a parse runs: they wait as one parse, the running one's result is dropped as
/// stale, and only the last text's is taken.
#[test]
fn fifty_fast_keystrokes_make_two_parses_and_the_last_one_lands() {
    let mut text = String::from("fn a() {\n    let n = 0;\n}\n");
    let mut h = fresh("rust", &text);
    let mut queue = ParseQueue::default();
    let (mut parses, mut applied) = (0, 0);

    let at = text.find("0;").unwrap();
    let edit = replace(&mut text, at, at, "1");
    h.edit_tree(Some(edit), &Rope::from(text.as_str()));
    assert!(queue.request(), "the first keystroke starts a parse");
    let mut running = h.background_parse();
    for i in 1..50 {
        let at = text.find(';').unwrap();
        let edit = replace(&mut text, at, at, &(i % 10).to_string());
        h.edit_tree(Some(edit), &Rope::from(text.as_str()));
        assert!(!queue.request(), "keystroke {i} waits behind the running parse");
    }
    while let Some(job) = running.take() {
        parses += 1;
        if h.apply_parsed(job.run().unwrap()) {
            applied += 1;
        }
        if queue.finish() {
            running = h.background_parse();
        }
    }
    assert_eq!(parses, 2, "the one that ran, and one for all the keystrokes during it");
    assert_eq!(applied, 1, "the first was for an older text and was dropped");
    assert_matches_fresh(&h, "rust", &text, "after the last parse");
    assert!(queue.request(), "the queue is idle again");
}
