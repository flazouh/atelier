//! One watch per pull request, shared by every card that shows it: it reads the pull request and its
//! checks off the UI thread at the pace `poll` sets. It reads only while a card of it is drawn in an
//! active window: each card tells it when it draws, and a wait that no draw followed pauses it. The next
//! draw in an active window reads at once.
use std::{collections::HashMap, sync::Arc, time::Duration};
use gpui_kit::{App, AppContext, Context, Entity, Global, Task, WeakEntity};
use lathe_forge::{Check, CheckStatus, Forge, ForgeError, Pull, PullRef, PullState};
use super::poll::{Seen, next_delay};

pub struct PullWatch {
    reference: PullRef,
    forge: Arc<dyn Forge>,
    pub pull: Option<Pull>,
    pub checks: Vec<Check>,
    /// Why the last read failed, while reads fail.
    pub unread: Option<gpui_kit::SharedString>,
    failure: Option<Duration>,
    /// The wait before the next read; `None` when the watch reads no more on its own.
    pub next: Option<Duration>,
    /// Draws of a card of it so far, and how many there were at the last read.
    drawn: u64,
    drawn_at_read: u64,
    /// Whether the window was active at the last draw.
    active: bool,
    /// Waiting for a card to draw before it reads again.
    paused: bool,
    polling: Task<()>,
}

/// The watches of this app, by pull request.
#[derive(Default)]
struct Watches(HashMap<PullRef, WeakEntity<PullWatch>>);
impl Global for Watches {}

/// The watch of `reference`: the one already there, or a new one that reads at once.
pub fn watch(reference: PullRef, forge: Arc<dyn Forge>, cx: &mut App) -> Entity<PullWatch> {
    let known = cx.default_global::<Watches>().0.get(&reference).and_then(WeakEntity::upgrade);
    if let Some(watch) = known {
        return watch;
    }
    let made = cx.new(|cx| {
        let mut watch = PullWatch {
            reference: reference.clone(),
            forge,
            pull: None,
            checks: Vec::new(),
            unread: None,
            failure: None,
            next: None,
            drawn: 0,
            drawn_at_read: 0,
            active: true,
            paused: false,
            polling: Task::ready(()),
        };
        watch.read_now(cx);
        watch
    });
    cx.default_global::<Watches>().0.insert(reference, made.downgrade());
    made
}

/// What one read got.
type Got = (Result<Pull, ForgeError>, Result<Vec<Check>, ForgeError>);

impl PullWatch {
    /// A card of it drew, in a window that is active or not. A paused watch reads at once.
    pub fn drawn(&mut self, active: bool, cx: &mut Context<Self>) {
        self.drawn += 1;
        self.active = active;
        if self.paused && active {
            self.paused = false;
            self.read_now(cx);
        }
    }

    /// Reads now, and then at the pace the reads set, while a card of it draws.
    pub fn read_now(&mut self, cx: &mut Context<Self>) {
        self.paused = false;
        let (forge, reference) = (self.forge.clone(), self.reference.clone());
        self.polling = cx.spawn(async move |this, cx| {
            loop {
                let (forge, reference) = (forge.clone(), reference.clone());
                let got: Got = cx.background_spawn(async move { (forge.pull(&reference), forge.checks(&reference)) }).await;
                let Ok(Some(wait)) = this.update(cx, |watch, cx| watch.got(got, cx)) else { break };
                cx.background_executor().timer(wait).await;
                // No card drew since the read, or the window is not active: wait for the next draw.
                let go_on = this.update(cx, |watch, _| {
                    let seen = watch.drawn > watch.drawn_at_read && watch.active;
                    watch.paused = !seen;
                    seen
                });
                if !matches!(go_on, Ok(true)) {
                    break;
                }
            }
        });
    }

    fn got(&mut self, (pull, checks): Got, cx: &mut Context<Self>) -> Option<Duration> {
        self.drawn_at_read = self.drawn;
        let seen = match pull {
            Ok(pull) => {
                if let Ok(checks) = checks {
                    self.checks = checks;
                }
                let running = pull.checks.running > 0 || self.checks.iter().any(|c| c.status != CheckStatus::Done);
                let seen = match pull.state {
                    PullState::Merged | PullState::Closed => Seen::Settled,
                    PullState::Open | PullState::Draft => Seen::Open { checks_running: running },
                };
                self.pull = Some(pull);
                self.unread = None;
                seen
            }
            Err(error) => {
                self.unread = Some(error.to_string().into());
                Seen::Failed(error)
            }
        };
        let next = next_delay(self.failure, &seen);
        self.failure = if matches!(seen, Seen::Failed(_)) { next } else { None };
        self.next = next;
        cx.notify();
        next
    }
}
