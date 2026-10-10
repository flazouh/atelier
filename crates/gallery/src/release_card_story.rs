//! The release card: what is new in one version, as a small modal. The picture is the changelog's grain gradient
//! made for this shape, with the app's name and the version over it. The notes are the changelog sheet's: a bold lead
//! and a muted text, no icon. It shows inline here, as the panel a `Modal`
//! made with `flush` gives it; the app opens it by itself the first time a new version starts, and from the version
//! in the status bar.
use atelier_ui::{ActiveTheme, ReleaseCard, ReleaseNote};
use gpui_kit::{App, IntoElement, ParentElement, SharedString, Styled, div, px};

fn card() -> ReleaseCard {
    ReleaseCard::new("release-card-story", "atelier", "0.1.16")
        .title("What\u{2019}s new in atelier")
        .date(Some(SharedString::from("Version 0.1.16 \u{b7} 10 October 2026")))
        .notes([
            ReleaseNote::new("Usage is a full view.", "The same sidebar and main pane as the rest of the app, with charts per session."),
            ReleaseNote::new("SSH folders open again.", "A folder on another computer opens with no helper error."),
            ReleaseNote::new("Pull request chips.", "A chip opens the pull requests page, not a side bar."),
        ])
        .on_secondary(|_, _| {})
        .on_primary(|_, _| {})
}

/// The card in the panel a modal gives it.
fn panel(card: ReleaseCard, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div().flex().justify_center().py(px(24.)).bg(theme.background).child(
        div()
            .w(px(440.))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.popover)
            .shadow(atelier_ui::theme::popover_shadow(theme))
            .child(card),
    )
}

pub fn release_card_story(cx: &App) -> impl IntoElement {
    div().flex().flex_col().gap(px(16.)).child(panel(card(), cx))
}
