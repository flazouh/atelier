//! The changelog sheet: a gradient band titled Changelog, then the releases stacked, each with its version and date at the
//! left and its notes at the right. No icons, no kinds, no colours, no buttons: a close button at the band's corner. It
//! shows inline here, as the panel of a `Modal` made with `flush`. The app opens it from the version in the status bar, and
//! by itself once, the first time the new version starts after an update.
use atelier_ui::{ActiveTheme, ReleaseNote, ReleaseSheet, ReleaseVersion};
use gpui_kit::{App, IntoElement, ParentElement, SharedString, Styled, div, px};

fn notes() -> Vec<ReleaseNote> {
    vec![
        ReleaseNote::new("The changelog.", "Each version shows its date and its notes in plain text, with our gradient on top."),
        ReleaseNote::new("Updates.", "An update downloads in the background, and a button in the title bar says when it is ready."),
    ]
}

fn earlier() -> Vec<ReleaseVersion> {
    vec![
        ReleaseVersion::new(
            "0.1.8",
            [
                ReleaseNote::new("The composer's stop.", "The stop icon is a little smaller and red."),
                ReleaseNote::new("The session header.", "The Stop button is gone. The composer stops a turn."),
            ],
        )
        .date(Some(SharedString::from("Oct 9, 2026"))),
        ReleaseVersion::new("0.1.7", [ReleaseNote::new("Tall panels.", "A panel stays inside the window and scrolls.")])
            .date(Some(SharedString::from("Oct 9, 2026"))),
    ]
}

/// The sheet in a panel as a modal gives it: the rounded corners the band must meet.
fn panel(sheet: ReleaseSheet, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div().flex().justify_center().py(px(24.)).bg(theme.background).child(
        div()
            .w(px(860.))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.popover)
            .shadow(atelier_ui::theme::popover_shadow(theme))
            .child(sheet),
    )
}

pub fn changelog_story(cx: &App) -> impl IntoElement {
    let sheet = ReleaseSheet::new("changelog-sheet", "0.1.9")
        .date(Some(SharedString::from("Oct 9, 2026")))
        .notes(notes())
        .earlier(earlier())
        .on_close(|_, _| {});
    div().flex().flex_col().gap(px(16.)).child(panel(sheet, cx))
}
