//! A language's queries compile once per process (PATCHES.md, patch 3). This file holds one test, so
//! no other test in its process compiled Rust's queries before it.
#![cfg(feature = "tree-sitter-languages")]

use std::time::{Duration, Instant};

use gpui_component::highlighter::{HighlightTheme, SyntaxHighlighter};
use ropey::Rope;

#[test]
fn a_second_highlighter_shares_the_first_ones_queries() {
    let started = Instant::now();
    let mut first = SyntaxHighlighter::new("rust");
    let compiled = started.elapsed();
    let again = (0..5)
        .map(|_| {
            let started = Instant::now();
            drop(SyntaxHighlighter::new("rust"));
            started.elapsed()
        })
        .min()
        .unwrap_or(Duration::MAX);
    assert!(again * 10 < compiled, "compiled in {compiled:?}, again in {again:?}");
    let mut second = SyntaxHighlighter::new("rust");
    let text = Rope::from("fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n");
    assert!(first.update(None, &text, None) && second.update(None, &text, None));
    let theme = HighlightTheme::default_dark();
    let all = 0..text.len();
    assert_eq!(first.styles(&all, theme.as_ref()), second.styles(&all, theme.as_ref()));
}
