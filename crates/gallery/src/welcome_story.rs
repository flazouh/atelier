//! The welcome page: the first thing a new reader sees, over the whole window. The gradient fills it, the mark stands
//! at the top left, the title and one line stream in at the middle as a message does, and one button stands alone at
//! the bottom right. It shows here in a frame of a window's shape; the app covers its window with it at a reader's
//! first start, until they press the button.
use atelier_ui::WelcomePage;
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};

pub fn welcome_story() -> impl IntoElement {
    div().w_full().max_w(px(1120.)).h(px(700.)).rounded(px(12.)).overflow_hidden().child(WelcomePage::new("welcome-story").on_continue(|_, _| {}))
}
