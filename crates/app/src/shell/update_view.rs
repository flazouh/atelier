//! The update's own look: a chip in the title bar while an update downloads and when it is ready, and one panel with the
//! changelog and the choice to restart or wait. Sparkle's own windows are not used.
use atelier_ui::{
    ReleaseNote, ReleaseSheet,
    button::{Button, ButtonSize, ButtonVariant},
    modal::Modal,
    theme::ActiveTheme,
    typography::TextSize,
};
use gpui_kit::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div};

use super::structs::Shell;
use crate::updater::{UpdateState, release_notes};

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
            UpdateState::Idle | UpdateState::Checking { .. } => self.whats_new.as_ref().map(|_| {
                let this = cx.entity().downgrade();
                Button::new("whats-new-chip")
                    .debug_name("whats-new-chip")
                    .label("What's new")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(move |_, _, cx| drop(this.update(cx, |shell, cx| shell.show_whats_new(cx))))
                    .into_any_element()
            }),
        }
    }

    /// The panel over the window while the reader reads the changelog: Later keeps the update for the next quit, and
    /// Restart installs it now. Escape and a press outside are Later.
    pub(super) fn update_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.whats_new_open {
            return self.whats_new_panel(cx);
        }
        let UpdateState::Ready { version, notes } = &self.update else { return None };
        if !self.update_modal {
            return None;
        }
        let (later, close, install) = (cx.entity().downgrade(), cx.entity().downgrade(), cx.entity().downgrade());
        let version: SharedString = if version.is_empty() { "the new version".into() } else { version.clone().into() };
        let mut lines = release_notes(notes);
        if lines.is_empty() {
            lines.push(crate::updater::NoteLine { lead: NO_NOTES.into(), text: String::new() });
        }
        let sheet = ReleaseSheet::new("update-sheet", version)
            .notes(lines.into_iter().map(|line| ReleaseNote::new(line.lead, line.text)))
            .on_later(move |_, cx| drop(later.update(cx, |shell, cx| shell.update_later(cx))))
            .on_install(move |_, cx| drop(install.update(cx, |shell, cx| shell.update_install(cx))));
        Some(
            Modal::new("update-ready")
                .width(520.)
                .flush()
                .focus(&self.update_focus)
                .on_close(move |_, cx| drop(close.update(cx, |shell, cx| shell.update_later(cx))))
                .child(div().track_focus(&self.update_focus).child(sheet))
                .into_any_element(),
        )
    }

    /// The changelog of the update this version came from, after the restart: the same sheet with only Close.
    fn whats_new_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let record = self.whats_new.as_ref()?;
        let (close, done) = (cx.entity().downgrade(), cx.entity().downgrade());
        let mut lines = release_notes(&record.notes);
        if lines.is_empty() {
            lines.push(crate::updater::NoteLine { lead: NO_NOTES.into(), text: String::new() });
        }
        let sheet = ReleaseSheet::new("whats-new-sheet", SharedString::from(record.version.clone()))
            .notes(lines.into_iter().map(|line| ReleaseNote::new(line.lead, line.text)))
            .labels("Close", "")
            .on_later(move |_, cx| drop(done.update(cx, |shell, cx| shell.dismiss_whats_new(cx))));
        Some(
            Modal::new("whats-new")
                .width(520.)
                .flush()
                .focus(&self.update_focus)
                .on_close(move |_, cx| drop(close.update(cx, |shell, cx| shell.dismiss_whats_new(cx))))
                .child(div().track_focus(&self.update_focus).child(sheet))
                .into_any_element(),
        )
    }
}
