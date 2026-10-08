//! Updates in the window: the Check for Updates action, the update's own look (a chip in the title bar, and one panel with
//! the changelog and the restart), and the question put to the reader when an installed update wants to restart the app
//! while a tab holds unsaved edits.
use futures_util::StreamExt;
use gpui_kit::{AppContext as _, Context, PromptLevel, Window};

use super::{helpers::settings_path, structs::{CheckForUpdates, Shell}};
use crate::updater::{
    remember_on_ready, remembered, running_version,
    CHECKING_NOTICE, CheckOutcome, DOWNLOADING_NOTICE, Question, Reaction, Remembered, RelaunchRequest, Requests, UNAVAILABLE_NOTICE,
    UPDATE_LATER_NOTICE, UPDATE_WAITS_NOTICE, UpdateEvent, UpdateEvents, UpdateState, Updater,
};

impl Shell {
    /// The app menu's Check for Updates…: an update that is ready opens in front, one that downloads says so, and else the
    /// updater looks and the answer comes as a notice or as the update. A build that cannot update says so in a notice.
    pub fn check_for_updates(&mut self, _: &CheckForUpdates, _: &mut Window, cx: &mut Context<Self>) {
        match &self.update {
            UpdateState::Ready { .. } => return self.show_update(cx),
            UpdateState::Downloading { .. } | UpdateState::Installing => return self.say(DOWNLOADING_NOTICE.to_string(), cx),
            UpdateState::Idle | UpdateState::Checking { .. } => {}
        }
        let outcome = cx.try_global::<Updater>().map_or(CheckOutcome::Unavailable, Updater::check_now);
        if outcome == CheckOutcome::Unavailable {
            self.say(UNAVAILABLE_NOTICE.to_string(), cx);
            return;
        }
        self.update = UpdateState::Checking { asked: true };
        self.say(CHECKING_NOTICE.to_string(), cx);
    }

    /// What the updater tells: the state moves, and the window says or shows what the move asks for.
    pub fn update_event(&mut self, event: UpdateEvent, cx: &mut Context<Self>) {
        let before = self.update.clone();
        let (state, reaction) = std::mem::take(&mut self.update).apply(event);
        self.update = state;
        // An update that is ready may install at the next quit: its changelog is kept, for that start to show once.
        if let Some(record) = remember_on_ready(&before, &self.update) {
            Self::keep_whats_new(Some(record), cx);
        }
        match reaction {
            Reaction::Nothing => {}
            Reaction::Say(words) => self.say(words, cx),
            Reaction::Show => self.update_modal = true,
        }
        cx.notify();
    }

    /// What the settings kept of the update this version came from: shown once as a chip. A kept changelog of a version not
    /// installed yet waits, and one of an older version goes.
    pub(super) fn remembered_at_start(saved: &atelier_settings::Settings, cx: &mut Context<Self>) -> Option<atelier_settings::WhatsNew> {
        let kept = saved.whats_new.as_ref()?;
        match remembered(&kept.version, &running_version()) {
            Remembered::Show => Some(kept.clone()),
            Remembered::Wait => None,
            Remembered::Forget => {
                Self::keep_whats_new(None, cx);
                None
            }
        }
    }

    /// Writes the kept changelog (or its removal) to the settings, off the UI thread.
    fn keep_whats_new(record: Option<atelier_settings::WhatsNew>, cx: &mut Context<Self>) {
        if let Some(path) = settings_path() {
            cx.background_spawn(async move {
                if let Err(error) = atelier_settings::update(&path, |s| s.whats_new = record) {
                    eprintln!("could not keep the changelog of the update: {error}");
                }
            })
            .detach();
        }
    }

    /// Opens the changelog of the update this version came from.
    pub fn show_whats_new(&mut self, cx: &mut Context<Self>) {
        if self.whats_new.is_some() {
            self.whats_new_open = true;
            cx.notify();
        }
    }

    /// Closes it for good: the chip goes, and the settings forget the changelog.
    pub fn dismiss_whats_new(&mut self, cx: &mut Context<Self>) {
        self.whats_new_open = false;
        if self.whats_new.take().is_some() {
            Self::keep_whats_new(None, cx);
        }
        cx.notify();
    }

    /// Puts the update, with its changelog, in front of the reader.
    pub fn show_update(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update, UpdateState::Ready { .. }) {
            self.update_modal = true;
            cx.notify();
        }
    }

    /// The reader chose to restart: the downloaded update installs. If a tab holds unsaved edits, the question below comes first.
    pub fn update_install(&mut self, cx: &mut Context<Self>) {
        if let Some(updater) = cx.try_global::<Updater>() {
            updater.install();
        }
        self.update_modal = false;
        self.update = UpdateState::Installing;
        cx.notify();
    }

    /// The reader chose to wait: the update installs when the app quits.
    pub fn update_later(&mut self, cx: &mut Context<Self>) {
        if let Some(updater) = cx.try_global::<Updater>() {
            updater.later();
        }
        self.update_modal = false;
        self.update = UpdateState::Idle;
        self.say(UPDATE_LATER_NOTICE.to_string(), cx);
    }

    /// The updater waits to restart the app so that an update installs. With nothing unsaved it restarts at once;
    /// else the reader is asked, and the restart waits for the answer. A no, or a window that closes first, keeps
    /// the app and its edits as they are.
    pub fn relaunch_for_update(&mut self, request: Box<dyn RelaunchRequest>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(held) = Updater::hold(request, self.unsaved(cx)) else { return };
        let question = held.question();
        let answer = window.prompt(PromptLevel::Warning, &question.title(), Some(question.detail()), &Question::BUTTONS, cx);
        cx.spawn(async move |this, cx| {
            let restart = answer.await == Ok(0);
            held.answer(restart);
            if !restart {
                _ = this.update(cx, |this, cx| {
                    this.update = UpdateState::Idle;
                    this.say(UPDATE_WAITS_NOTICE.to_string(), cx);
                });
            }
        })
        .detach();
    }

    /// Takes the restarts the platform updater asks for, one at a time, for as long as the window lives.
    pub fn serve_relaunches(&mut self, mut requests: Requests, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(request) = requests.next().await {
                if this.update_in(cx, |this, window, cx| this.relaunch_for_update(request, window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Takes what the platform updater tells about an update, for as long as the window lives.
    pub fn serve_updates(&mut self, mut events: UpdateEvents, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |this, cx| this.update_event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}
