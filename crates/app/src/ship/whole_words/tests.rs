use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window, div, px, size};

use super::*;

struct Line;

impl Render for Line {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(170.)).text_size(px(12.)).child(whole_words("Pushed feat/add-subtract-function to origin"))
    }
}

/// D1: a line that names a branch never breaks inside the name. The line wraps before it.
#[gpui_kit::test]
fn a_branch_name_stays_whole_and_the_line_wraps_before_it(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (_line, cx) = cx.add_window_view(|_, _| Line);
    cx.simulate_resize(size(px(400.), px(200.)));
    cx.run_until_parked();
    let first = cx.debug_bounds("word-0").expect("Pushed is drawn");
    let branch = cx.debug_bounds("word-1").expect("the branch is drawn");
    assert_eq!(branch.size.height, first.size.height, "the branch is on one line: {branch:?}");
    assert!(branch.top() > first.top(), "the line wraps before the branch: {first:?} {branch:?}");
    assert_eq!(branch.left(), first.left(), "the branch starts its own line");
}

#[test]
fn the_words_are_split_at_spaces_only() {
    assert_eq!(split("Pushed feat/a-b  to origin"), ["Pushed", "feat/a-b", "to", "origin"]);
}
