use std::cell::RefCell;

use super::super::{NativeRelaunch, RelaunchRequest, RequestSender, SparkleDriver, UpdateDriver};

unsafe extern "C" {
    fn atelier_updater_start(callback: extern "C" fn()) -> bool;
    fn atelier_updater_check();
    fn atelier_updater_proceed();
    fn atelier_updater_decline();
}

thread_local! {
    /// Where Sparkle's call, always on the main thread, sends the restart it waits on.
    static REQUESTS: RefCell<Option<RequestSender>> = const { RefCell::new(None) };
}

/// Sparkle has an update ready and waits for the app to say it may restart.
extern "C" fn relaunch_requested() {
    REQUESTS.with(|requests| {
        let Some(sender) = requests.borrow().clone() else { return };
        if let Err(lost) = sender.unbounded_send(Box::new(NativeRelaunch)) {
            lost.into_inner().decline();
        }
    });
}

impl SparkleDriver {
    /// Starts Sparkle, when this build is the released app, which carries it. A build from source does not.
    pub fn start(requests: RequestSender) -> Option<Self> {
        REQUESTS.with(|slot| *slot.borrow_mut() = Some(requests));
        // SAFETY: the callback has the signature the native side expects, and runs on the main thread, as this does.
        unsafe { atelier_updater_start(relaunch_requested) }.then_some(SparkleDriver)
    }
}

impl UpdateDriver for SparkleDriver {
    fn available(&self) -> bool {
        true
    }

    fn check(&self) {
        // SAFETY: Sparkle was started, and this runs on the main thread.
        unsafe { atelier_updater_check() }
    }
}

impl RelaunchRequest for NativeRelaunch {
    fn proceed(self: Box<Self>) {
        // SAFETY: the native side holds the install handler Sparkle gave it, and this runs on the main thread.
        unsafe { atelier_updater_proceed() }
    }

    fn decline(self: Box<Self>) {
        // SAFETY: as in `proceed`.
        unsafe { atelier_updater_decline() }
    }
}
