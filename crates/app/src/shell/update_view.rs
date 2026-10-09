//! The update's own look: the design system's update button in the title bar (a ring that fills while an update
//! downloads, then "Update to vX"), and the changelog sheet
//! (the same one the version in the status bar opens, and the first start after an update opens by itself). Sparkle's own
//! windows are not used.
use atelier_ui::{ReleaseNote, ReleaseSheet, ReleaseVersion, UpdateButton, modal::Modal};
use gpui_kit::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Window, div};

use super::{helpers::sheet_width, structs::Shell};
use crate::{
    changelog,
    updater::{UpdateState, is_older, release_date, release_notes, running_version},
};

/// What a changelog the feed did not carry says.
const NO_NOTES: &str = "This version has no release notes.";

/// The notes of a release as the sheet lists them: one when the changelog has none, so the release is not empty.
fn notes_of(markdown: &str) -> Vec<ReleaseNote> {
    let mut notes: Vec<ReleaseNote> = release_notes(markdown).into_iter().map(|line| ReleaseNote::new(line.lead, line.text)).collect();
    if notes.is_empty() {
        notes.push(ReleaseNote::new(NO_NOTES, ""));
    }
    notes
}

/// A release as the sheet draws it: its version, its date when its notes carry one, and its notes.
type Release = (SharedString, Option<SharedString>, Vec<ReleaseNote>);

fn release_of(version: &str, markdown: &str) -> Release {
    (SharedString::from(version.to_string()), release_date(markdown).map(SharedString::from), notes_of(markdown))
}

fn listed((version, date, notes): Release) -> ReleaseVersion {
    ReleaseVersion::new(version, notes).date(date)
}

impl Shell {
    /// The room the chip takes in the title bar now: none when there is no chip.
    pub(super) fn update_chip_room(&self) -> f32 {
        match &self.update {
            UpdateState::Downloading { .. } | UpdateState::Ready { .. } | UpdateState::Installing => super::types::UPDATE_ROOM,
            UpdateState::Idle | UpdateState::Checking { .. } => 0.,
        }
    }

    /// The chip at the right of the title bar, and the only place that builds it: how far the download is, then the button
    /// that restarts and installs once the update is ready. Its look is one value here, so it can be swapped for another.
    pub(super) fn update_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let button = UpdateButton::new("update-button");
        match &self.update {
            UpdateState::Downloading { fraction, .. } => {
                Some(button.downloading(*fraction as f32, format!("Updating {}%", (fraction * 100.).floor() as u32)).into_any_element())
            }
            UpdateState::Ready { version, .. } => {
                let this = cx.entity().downgrade();
                let label = if version.is_empty() { "Update and restart".to_string() } else { format!("Update to v{version}") };
                Some(
                    button
                        .ready(label)
                        .on_click(move |_, cx| drop(this.update(cx, |shell, cx| shell.update_install(cx))))
                        .into_any_element(),
                )
            }
            UpdateState::Installing => Some(button.restarting("Restarting…").into_any_element()),
            UpdateState::Idle | UpdateState::Checking { .. } => None,
        }
    }

    /// The sheet over the window while the reader reads a changelog: the one this version came from, which opens by itself
    /// at the first start after an update, or the one the version in the status bar opens.
    pub(super) fn update_panel(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.whats_new_open {
            return self.whats_new_panel(window, cx);
        }
        if self.changelog_open {
            return self.changelog_panel(window, cx);
        }
        None
    }

    /// The changelog of the update this version came from, after the restart: its notes first, the older releases under.
    fn whats_new_panel(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let record = self.whats_new.as_ref()?;
        let date = release_date(&record.notes).or_else(|| changelog::notes_of(&record.version).and_then(release_date));
        let earlier = changelog::releases()
            .iter()
            .filter(|(version, _)| is_older(version, &record.version))
            .map(|(version, markdown)| release_of(version, markdown))
            .collect();
        let current = (SharedString::from(record.version.clone()), date.map(SharedString::from), notes_of(&record.notes));
        Some(self.changelog_modal("whats-new", current, earlier, Self::dismiss_whats_new, window, cx))
    }

    /// The notes of the version that runs, with every earlier version under them.
    fn changelog_panel(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let running = running_version();
        let current = release_of(&running, changelog::notes_of(&running).unwrap_or_default());
        let earlier = changelog::releases().iter().filter(|(version, _)| *version != running.as_str()).map(|(version, markdown)| release_of(version, markdown)).collect();
        Some(self.changelog_modal("changelog", current, earlier, Self::close_changelog, window, cx))
    }

    /// The changelog sheet in its panel: 860 design pixels wide, less in a narrow window; `close` runs on Close, on Escape
    /// and on a press outside the panel.
    fn changelog_modal(
        &self,
        id: &'static str,
        (version, date, notes): Release,
        earlier: Vec<Release>,
        close: fn(&mut Shell, &mut Context<Shell>),
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (on_modal, on_sheet) = (cx.entity().downgrade(), cx.entity().downgrade());
        let sheet = ReleaseSheet::new(SharedString::from(format!("{id}-sheet")), version)
            .title("Changelog")
            .date(date)
            .notes(notes)
            .earlier(earlier.into_iter().map(listed))
            .on_close(move |_, cx| drop(on_sheet.update(cx, close)));
        Modal::new(id)
            .debug_name("update-panel")
            .width(sheet_width(atelier_ui::scale::design(window.viewport_size().width)))
            .flush()
            .focus(&self.update_focus)
            .on_close(move |_, cx| drop(on_modal.update(cx, close)))
            .child(div().track_focus(&self.update_focus).child(sheet))
            .into_any_element()
    }
}
