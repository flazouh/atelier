//! The update's own look: a chip in the title bar while an update downloads and when it is ready, and one panel with the
//! changelog and the choice to restart or wait. Sparkle's own windows are not used.
use atelier_ui::{
    AgentText,
    button::{Button, ButtonSize, ButtonVariant},
    modal::Modal,
    theme::ActiveTheme,
    typography::TextSize,
};
use atelier_ui::scale::px;
use gpui_kit::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div};

use super::structs::Shell;
use crate::updater::UpdateState;

/// What a changelog the feed did not carry says.
const NO_NOTES: &str = "This version has no release notes.";

impl Shell {
    /// The chip at the right of the title bar: how far the download is, and the button to open the update once it is ready.
    pub(super) fn update_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let muted = cx.theme().muted_foreground;
        match &self.update {
            UpdateState::Downloading { fraction, .. } => Some(
                div()
                    .debug_selector(|| "update-progress".into())
                    .text_size(TextSize::Xs.font_size())
                    .text_color(muted)
                    .child(format!("Updating {}%", (fraction * 100.).floor() as u32))
                    .into_any_element(),
            ),
            UpdateState::Ready { .. } => {
                let this = cx.entity().downgrade();
                Some(
                    Button::new("update-chip")
                        .debug_name("update-chip")
                        .label("Update ready")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .on_click(move |_, _, cx| drop(this.update(cx, |shell, cx| shell.show_update(cx))))
                        .into_any_element(),
                )
            }
            UpdateState::Installing => {
                Some(div().text_size(TextSize::Xs.font_size()).text_color(muted).child("Installing…").into_any_element())
            }
            UpdateState::Idle | UpdateState::Checking { .. } => None,
        }
    }

    /// The panel over the window while the reader reads the changelog: Later keeps the update for the next quit, and Restart
    /// installs it now. Escape and a press outside are Later.
    pub(super) fn update_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let UpdateState::Ready { version, notes } = &self.update else { return None };
        if !self.update_modal {
            return None;
        }
        let theme = cx.theme().clone();
        let (later, close, install) = (cx.entity().downgrade(), cx.entity().downgrade(), cx.entity().downgrade());
        let title = if version.is_empty() { "A new version of atelier is ready".to_string() } else { format!("atelier {version} is ready") };
        let notes: SharedString = if notes.trim().is_empty() { NO_NOTES.into() } else { notes.clone().into() };
        let body = div()
            .debug_selector(|| "update-notes".into())
            .id("update-notes")
            .max_h(px(320.))
            .overflow_y_scroll()
            .child(AgentText::new("update-notes-text", notes));
        let actions = div()
            .flex()
            .justify_end()
            .gap(px(8.))
            .child(
                Button::new("update-later")
                    .debug_name("update-later")
                    .label("Later")
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| drop(later.update(cx, |shell, cx| shell.update_later(cx)))),
            )
            .child(
                Button::new("update-restart")
                    .debug_name("update-restart")
                    .label("Restart and update")
                    .variant(ButtonVariant::Primary)
                    .on_click(move |_, _, cx| drop(install.update(cx, |shell, cx| shell.update_install(cx)))),
            );
        Some(
            Modal::new("update-ready")
                .width(520.)
                .focus(&self.update_focus)
                .on_close(move |_, cx| drop(close.update(cx, |shell, cx| shell.update_later(cx))))
                .child(
                    div()
                        .track_focus(&self.update_focus)
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .p(px(20.))
                        .child(div().text_size(TextSize::Lg.font_size()).text_color(theme.foreground).child(title))
                        .child(body)
                        .child(actions),
                )
                .into_any_element(),
        )
    }
}
