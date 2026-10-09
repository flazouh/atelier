//! Updates in the window: the Check for Updates action, the update's own look (a chip in the title bar), the changelog the
//! first start after an update opens, the install at quit, and the question put to the reader when an installed update
//! wants to restart the app while a tab holds unsaved edits.
use futures_util::StreamExt;
use gpui_kit::{AppContext as _, Context, PromptLevel, Window};

use super::{helpers::settings_path, structs::{CheckForUpdates, Shell}};
use crate::updater::{
    remember_on_ready, remembered, running_version,
    CHECKING_NOTICE, CheckOutcome, DOWNLOADING_NOTICE, Question, Reaction, Remembered, RelaunchRequest, Requests, UNAVAILABLE_NOTICE,
    UPDATE_READY_NOTICE, UPDATE_WAITS_NOTICE, UpdateEvent, UpdateEvents, UpdateState, Updater,
};

impl Shell {
    /// The app menu's Check for Updates…: an update that is ready or one that downloads says so, and else the updater looks and
    /// the answer comes as a notice or as the update. A build that cannot update says so in a notice.
    pub fn check_for_updates(&mut self, _: &CheckForUpdates, _: &mut Window, cx: &mut Context<Self>) {
        match &self.update {
            UpdateState::Ready { .. } => return self.say(UPDATE_READY_NOTICE.to_string(), cx),
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

    /// Where an update stands, in one word, for the control socket.
    pub fn update_state_word(&self) -> &'static str {
        match self.update {
            UpdateState::Idle => "idle",
            UpdateState::Checking { .. } => "checking",
            UpdateState::Downloading { .. } => "downloading",
            UpdateState::Ready { .. } => "ready",
            UpdateState::Installing => "installing",
        }
    }

    /// Whether a changelog sheet is open, for the control socket.
    pub fn changelog_shown(&self) -> bool {
        self.changelog_open || self.whats_new_open
    }

    /// What the updater tells: the state moves, and the window says what the move asks for.
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
        }
        cx.notify();
    }

    /// What the settings kept of the update this version came from: its changelog opens once, at start. A kept changelog of a version not
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

    /// Opens the changelog from the version in the title bar.
    pub fn show_changelog(&mut self, cx: &mut Context<Self>) {
        self.changelog_open = true;
        cx.notify();
    }

    pub fn close_changelog(&mut self, cx: &mut Context<Self>) {
        self.changelog_open = false;
        cx.notify();
    }

    /// Closes the changelog of the update this version came from for good: the settings forget it, so it never opens again.
    pub fn dismiss_whats_new(&mut self, cx: &mut Context<Self>) {
        self.whats_new_open = false;
        if self.whats_new.take().is_some() {
            Self::keep_whats_new(None, cx);
        }
        cx.notify();
    }

    /// The reader pressed the update button: the downloaded update installs. If a tab holds unsaved edits, the question below
    /// comes first.
    pub fn update_install(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.update, UpdateState::Ready { .. }) {
            return;
        }
        if let Some(updater) = cx.try_global::<Updater>() {
            updater.install();
        }
        self.update = UpdateState::Installing;
        cx.notify();
    }

    /// The app quits. An update that is ready and was never pressed installs now, as it did when the reader chose Later.
    /// The Mac side takes its reply out of a slot when it answers, so a second call, or one with no update waiting, does nothing.
    pub fn update_at_quit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update, UpdateState::Ready { .. })
            && let Some(updater) = cx.try_global::<Updater>()
        {
            updater.later();
        }
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
        // Once, here: the window serves the updater's events once, and what it learns decides what quitting does.
        self._subscriptions.push(cx.on_app_quit(|this, cx| {
            this.update_at_quit(cx);
            async {}
        }));
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
