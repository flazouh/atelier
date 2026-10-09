//! The pieces the Chat and Mail cards share: a labelled field, a note, a switch row and the row of buttons. They use the
//! sizes and colours of the Linear and GitHub cards beside them.
use atelier_ui::{
    Button, ButtonVariant, Switch, TextInput,
    scale::px,
    theme::Theme,
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    component::input::InputState, div, prelude::FluentBuilder,
};

use super::Checked;
use crate::control::marked;

/// A field under its label. The label says what to type; the placeholder says what it looks like.
pub(super) fn labelled(
    label: &str,
    id: &'static str,
    field: &Entity<InputState>,
    theme: &Theme,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .text_color(theme.muted_foreground)
                .child(SharedString::from(label.to_string())),
        )
        .child(TextInput::new(id, field).surface(theme.background))
        .into_any_element()
}

/// A line of small muted words, as the GitHub card has under its field.
pub(super) fn note(words: &str, theme: &Theme) -> AnyElement {
    div()
        .text_size(TextSize::Xs.font_size())
        .text_color(theme.muted_foreground)
        .child(SharedString::from(words.to_string()))
        .into_any_element()
}

/// A switch with its words at the left, as the Settings page's rows have it, and a line under it when there is one.
pub(super) fn switch_row(
    id: &'static str,
    label: &str,
    on: bool,
    under: Option<(&str, gpui_kit::Hsla)>,
    theme: &Theme,
    on_change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.))
                .min_h(px(32.))
                .child(
                    div()
                        .text_size(TextSize::Sm.font_size())
                        .text_color(theme.foreground)
                        .child(SharedString::from(label.to_string())),
                )
                .child(Switch::new(id, on).debug_name(id).on_change(on_change)),
        )
        .when_some(under, |d, (words, colour)| {
            d.child(
                div()
                    .text_size(TextSize::Xs.font_size())
                    .text_color(colour)
                    .child(SharedString::from(words.to_string())),
            )
        })
        .into_any_element()
}

/// The names a card's buttons are found by.
#[derive(Clone, Copy)]
pub(super) struct Buttons {
    pub test: &'static str,
    pub save: &'static str,
    pub forget: &'static str,
}

/// Test, Save, and Forget at the right once something is saved.
pub(super) fn actions(
    names: Buttons,
    check: &Checked,
    saved: bool,
    test: impl Fn(&mut Window, &mut App) + 'static,
    save: impl Fn(&mut Window, &mut App) + 'static,
    forget: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .flex_wrap()
        .child(marked(
            names.test,
            Button::new(names.test)
                .label("Test")
                .variant(ButtonVariant::Secondary)
                .disabled(*check == Checked::Checking)
                .on_click(move |_, window, cx| test(window, cx)),
        ))
        .child(marked(
            names.save,
            Button::new(names.save)
                .label("Save")
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, window, cx| save(window, cx)),
        ))
        .child(div().flex_1())
        .when(saved, |d| {
            d.child(marked(
                names.forget,
                Button::new(names.forget)
                    .label("Forget")
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, window, cx| forget(window, cx)),
            ))
        })
        .into_any_element()
}

/// What a field holds, trimmed.
pub(super) fn typed(field: &Entity<InputState>, cx: &App) -> String {
    field.read(cx).value().trim().to_string()
}
