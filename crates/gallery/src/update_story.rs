//! The update sheet, the panel that says a new version is ready: the grain picture drifting behind the version, then
//! the notes and the choice coming in one after the other. "Replay" opens it again, so the entrance can be watched
//! as often as needed. It shows inline here, as the panel of a [`Modal`](atelier_ui::Modal) made with `flush`.
use atelier_ui::{ActiveTheme, Button, ButtonVariant, IconName, ReleaseNote, ReleaseSheet, TextSize};
use gpui_kit::{App, ClickEvent, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

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

pub fn update_story(
    replays: usize,
    on_replay: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(Button::new("update-replay").label("Replay").variant(ButtonVariant::Secondary).on_click(on_replay))
                .child(
                    div()
                        .text_size(TextSize::Xs.font_size())
                        .font_weight(FontWeight::MEDIUM)
                        .opacity(0.6)
                        .child("The picture zooms in and then drifts for as long as the sheet is open."),
                ),
        )
        .child(
            div().flex().justify_center().py(px(32.)).bg(theme.background).child(
                div()
                    .w(px(520.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(theme.popover)
                    .shadow(atelier_ui::theme::popover_shadow(theme))
                    .child(
                        ReleaseSheet::new(("update-sheet", replays), "0.1.2")
                            .notes(notes())
                            .on_later(|_, _| {})
                            .on_install(|_, _| {}),
                    ),
            ),
        )
}
