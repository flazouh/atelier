//! The update sheet, the panel that says a new version is ready: the grain picture behind the version, the notes in a
//! dark panel, and the choice. It shows inline here, as the panel of a `Modal` made with `flush`.
use atelier_ui::{ActiveTheme, IconName, ReleaseNote, ReleaseSheet};
use gpui_kit::{App, IntoElement, ParentElement, Styled, div, px};

fn notes() -> Vec<ReleaseNote> {
    vec![
        ReleaseNote::new("Updates in our own window.", "A small button in the title bar opens this changelog.")
            .icon(IconName::Download),
        ReleaseNote::new("Restart when you choose.", "Press Later and the update installs when you quit.")
            .icon(IconName::Refresh),
        ReleaseNote::new("Notes always show.", "The changelog travels inside the update, so it is never missing.")
            .icon(IconName::Check),
    ]
}

pub fn update_story(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div().flex().justify_center().py(px(32.)).bg(theme.background).child(
                div()
                    .w(px(520.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(theme.popover)
                    .shadow(atelier_ui::theme::popover_shadow(theme))
                    .child(
                        ReleaseSheet::new("update-sheet", "0.1.2")
                            .notes(notes())
                            .on_later(|_, _| {})
                            .on_install(|_, _| {}),
                    ),
            ),
        )
}
