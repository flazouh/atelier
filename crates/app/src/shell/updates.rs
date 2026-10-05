//! Updates in the window: the Check for Updates action, and the question put to the reader when an installed
//! update wants to restart the app while a tab holds unsaved edits.
use futures_util::StreamExt;
use gpui_kit::{Context, PromptLevel, Window};

use super::structs::{CheckForUpdates, Shell};
use crate::updater::{CheckOutcome, Relaunch, RelaunchRequest, Requests, UNAVAILABLE_NOTICE, Updater};

impl Shell {
    /// The app menu's Check for Updates…: the updater looks and shows its own window. A build that cannot update
    /// says so in a notice.
    pub fn check_for_updates(&mut self, _: &CheckForUpdates, _: &mut Window, cx: &mut Context<Self>) {
        let outcome = cx.try_global::<Updater>().map_or(CheckOutcome::Unavailable, Updater::check_now);
        if outcome == CheckOutcome::Unavailable {
            self.say(UNAVAILABLE_NOTICE.to_string(), cx);
        }
    }

    /// The updater waits to restart the app so that an update installs. With nothing unsaved it restarts at once;
    /// else the reader is asked, and the restart waits for the answer. A no, or a window that closes first, keeps
    /// the app and its edits as they are.
    pub fn relaunch_for_update(&mut self, request: Box<dyn RelaunchRequest>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(held) = Updater::hold(request, self.unsaved(cx)) else { return };
        let relaunch = held.relaunch();
        let answer = window.prompt(PromptLevel::Warning, &relaunch.title(), Some(relaunch.detail()), &Relaunch::BUTTONS, cx);
        cx.spawn(async move |_, _| held.answer(answer.await == Ok(0))).detach();
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
}
