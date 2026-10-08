//! The update sheet, the panel that says a new version is ready: the grain picture behind the version, the notes in a
//! dark panel, and the choice. It shows inline here, as the panel of a `Modal` made with `flush`. A note has a kind (new,
//! improved, fixed) that gives its icon and colour. The second sheet is the changelog the version in the title bar opens: no
//! restart, Close at the top right, earlier versions under the notes.
use atelier_ui::{ActiveTheme, ReleaseKind, ReleaseNote, ReleaseSheet, ReleaseVersion};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div, px};

fn notes() -> Vec<ReleaseNote> {
    vec![
        ReleaseNote::new("Updates in our own window.", "A small button in the title bar opens this changelog.").kind(ReleaseKind::New),
        ReleaseNote::new("Restart when you choose.", "Press Later and the update installs when you quit.").kind(ReleaseKind::Improved),
        ReleaseNote::new("Notes always show.", "The changelog travels inside the update, so it is never missing.").kind(ReleaseKind::Fixed),
    ]
}

fn earlier() -> Vec<ReleaseVersion> {
    vec![
        ReleaseVersion::new(
            "0.1.4",
            [
                ReleaseNote::new("Updates find you sooner.", "Atelier looks every hour, and when you come back to it.").kind(ReleaseKind::New),
                ReleaseNote::new("The update sheet.", "The picture fills the sheet with round corners.").kind(ReleaseKind::Improved),
            ],
        ),
        ReleaseVersion::new(
            "0.1.3",
            [ReleaseNote::new("Claude is found again.", "Atelier reads your shell's PATH when it starts.").kind(ReleaseKind::Fixed)],
        ),
    ]
}

/// The sheet in a panel as a modal gives it: the rounded corners the picture must meet.
fn panel(sheet: ReleaseSheet, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div().flex().justify_center().py(px(24.)).bg(theme.background).child(
        div()
            .w(px(520.))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.popover)
            .shadow(atelier_ui::theme::popover_shadow(theme))
            .child(sheet),
    )
}

pub fn update_story(cx: &App) -> impl IntoElement {
    let restart = ReleaseSheet::new("update-sheet", "0.1.5").notes(notes()).on_later(|_, _| {}).on_install(|_, _| {});
    let changelog = ReleaseSheet::new("changelog-sheet", "0.1.5")
        .notes(notes())
        .earlier(earlier())
        .labels("Close", "")
        .on_later(|_, _| {});
    div().flex().flex_col().gap(px(16.)).child(panel(restart, cx)).child(panel(changelog, cx))
}
