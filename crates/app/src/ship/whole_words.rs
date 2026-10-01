//! The strip's words, set so that a line never breaks inside a word. gpui may break a line at `/`, so a
//! branch such as `feat/add-subtract-function` could split in two. Here each word is laid out whole, and
//! the line wraps between words, before the branch. Every strip line that names a branch goes through it.

use gpui_kit::{Div, InteractiveElement, ParentElement, SharedString, Styled, div, };
use beui::scale::px;

/// The words of `text`, each kept whole, wrapping between them. A word wider than the line is cut with an
/// ellipsis rather than broken.
pub fn whole_words(text: &str) -> Div {
    div().flex().flex_wrap().gap_x(px(4.)).min_w_0().children(split(text).into_iter().enumerate().map(|(i, word)| {
        div().debug_selector(move || format!("word-{i}")).max_w_full().truncate().child(word)
    }))
}

/// The words, split at spaces only.
fn split(text: &str) -> Vec<SharedString> {
    text.split_whitespace().map(|word| SharedString::from(word.to_string())).collect()
}

#[cfg(test)]
mod tests;
