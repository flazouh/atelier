//! Incremental injection layers (PATCHES.md, patch 1) against the ground truth: after each of a series
//! of seeded random edits, a highlighter updated with the edit has the same injection layers and the
//! same styles as one that parsed the same text from scratch.
#![cfg(feature = "tree-sitter-languages")]

use gpui_component::highlighter::{HighlightTheme, SyntaxHighlighter};
use ropey::Rope;
use tree_sitter::{InputEdit, Point};

/// The point of byte `offset` in `text`.
fn point_of(text: &str, offset: usize) -> Point {
    let before = &text[..offset];
    let row = before.matches('\n').count();
    let column = offset - before.rfind('\n').map_or(0, |n| n + 1);
    Point::new(row, column)
}

/// Each layer as `language start..end [ranges]`, with each range's points.
fn shapes(h: &SyntaxHighlighter) -> Vec<String> {
    h.injection_layer_ranges()
        .iter()
        .map(|(language, ranges, bytes)| {
            let ranges: Vec<String> = ranges
                .iter()
                .map(|r| {
                    format!(
                        "{}..{}@{}:{}-{}:{}",
                        r.start_byte, r.end_byte, r.start_point.row, r.start_point.column, r.end_point.row, r.end_point.column
                    )
                })
                .collect();
            format!("{language} {bytes:?} {ranges:?}")
        })
        .collect()
}

#[track_caller]
fn assert_incremental_matches_rebuild(language: &str, text: &str, snippets: &[&str], edits: usize, seed: u64) {
    let theme = HighlightTheme::default_dark();
    let mut state = seed;
    let mut next = move |n: usize| {
        // xorshift64: deterministic, so a failure replays.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n.max(1) as u64) as usize
    };
    let mut text = text.to_string();
    let mut h = SyntaxHighlighter::new(language);
    assert!(h.update(None, &Rope::from(text.as_str()), None));
    assert!(!h.injection_layer_ranges().is_empty(), "{language}: the fixture has injections");
    for step in 0..edits {
        let mut start = next(text.len() + 1);
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let (old_end, insert) = if next(3) == 0 {
            let mut end = (start + next(24)).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            (end, "")
        } else {
            (start, snippets[next(snippets.len())])
        };
        let (start_position, old_end_position) = (point_of(&text, start), point_of(&text, old_end));
        text.replace_range(start..old_end, insert);
        let new_end = start + insert.len();
        let edit = InputEdit {
            start_byte: start,
            old_end_byte: old_end,
            new_end_byte: new_end,
            start_position,
            old_end_position,
            new_end_position: point_of(&text, new_end),
        };
        let rope = Rope::from(text.as_str());
        assert!(h.update(Some(edit), &rope, None));
        let mut fresh = SyntaxHighlighter::new(language);
        assert!(fresh.update(None, &rope, None));
        let what = format!("{language}, edit {step} ({start}..{old_end} -> {insert:?})");
        let (got, want) = (shapes(&h), shapes(&fresh));
        if got != want {
            let only = |a: &[String], b: &[String]| a.iter().filter(|x| !b.contains(x)).cloned().collect::<Vec<_>>();
            panic!(
                "{what}: layers differ\n  only incremental: {:?}\n  only rebuild: {:?}\n  text: {text:?}",
                only(&got, &want),
                only(&want, &got)
            );
        }
        assert_eq!(
            h.styles(&(0..rope.len()), theme.as_ref()),
            fresh.styles(&(0..rope.len()), theme.as_ref()),
            "{what}: styles differ"
        );
    }
}

#[test]
fn rust_macros() {
    let text = indoc::indoc! {r#"
        fn a(v: u64) -> String {
            let s = format!("{} and {}", v, "x");
            println!("{s}");
            vec![1, 2, 3].len();
            s
        }
        macro_rules! twice { ($e:expr) => { $e; $e } }
        fn b() { twice!(a(1)); assert_eq!(1, 1); }
    "#};
    let snippets = ["x", "\n", "m!(a)", "(", ")", "\"", "format!(\"{}\", 1)", "!", "[", "]", "{", "}", " "];
    assert_incremental_matches_rebuild("rust", text, &snippets, 300, 0x9e37_79b9_7f4a_7c15);
}

#[test]
fn markdown_fences_and_inline() {
    let text = indoc::indoc! {r#"
        # Title with `code` and *emphasis*

        A paragraph with a [link](https://example.com) and **bold**.

        ```rust
        fn main() { println!("hi"); }
        ```

        - one `item`
        - two _items_

        <div>html block</div>

        ```js
        const a = `t`;
        ```
    "#};
    let snippets = ["x", "\n", "```", "```rust\n", "`", "*", "_", "[a](b)", "\n\n", "<b>", "# ", "- "];
    assert_incremental_matches_rebuild("markdown", text, &snippets, 300, 0x2545_f491_4f6c_dd1d);
}

#[test]
fn html_script_and_style() {
    let text = indoc::indoc! {r#"
        <html>
        <head><style>body { color: red; }</style></head>
        <body>
        <script>const a = 1; function f() { return a; }</script>
        <p>text</p>
        <script>let b = "</p>";</script>
        </body>
        </html>
    "#};
    let snippets = ["x", "\n", "<script>", "</script>", "<style>", "</style>", "{", "}", ";", "\"", "<", ">"];
    assert_incremental_matches_rebuild("html", text, &snippets, 300, 0xda94_2042_e4dd_58b5);
}

/// Past the cap on injected parses (512) only the first layers exist, and an edit must keep that set.
#[test]
fn rust_past_the_layer_cap() {
    let text: String = (0..560).map(|i| format!("fn f{i}() {{ m!({i}); }}\n")).collect();
    let snippets = ["m!(x)", "x", "\n", "(", ")", "fn g() { n!(1); }\n"];
    assert_incremental_matches_rebuild("rust", &text, &snippets, 40, 0x1234_5678_9abc_def1);
}
