use std::{cell::RefCell, ffi::{CStr, c_char}};

use super::super::{NativeRelaunch, RelaunchRequest, RequestSender, SparkleDriver, UpdateDriver, UpdateEvent, UpdateSender};

unsafe extern "C" {
    fn atelier_updater_start(relaunch: extern "C" fn(), event: extern "C" fn(*const c_char)) -> bool;
    fn atelier_updater_check(asked: bool);
    fn atelier_updater_install();
    fn atelier_updater_later();
    fn atelier_updater_proceed();
    fn atelier_updater_decline();
}

thread_local! {
    /// Where Sparkle's call, always on the main thread, sends the restart it waits on.
    static REQUESTS: RefCell<Option<RequestSender>> = const { RefCell::new(None) };
    /// Where Sparkle's calls, on the main thread, tell how an update goes.
    static EVENTS: RefCell<Option<UpdateSender>> = const { RefCell::new(None) };
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

/// The native side tells how an update goes, as one line of JSON.
extern "C" fn update_event(line: *const c_char) {
    if line.is_null() {
        return;
    }
    // SAFETY: the native side passes a NUL-terminated string that lives for this call.
    let text = unsafe { CStr::from_ptr(line) }.to_string_lossy();
    let Ok(event) = serde_json::from_str::<UpdateEvent>(&text) else { return };
    EVENTS.with(|events| {
        if let Some(sender) = events.borrow().as_ref() {
            drop(sender.unbounded_send(event));
        }
    });
}

impl SparkleDriver {
    /// Starts Sparkle, when this build is the released app, which carries it. A build from source does not.
    pub fn start(requests: RequestSender, events: UpdateSender) -> Option<Self> {
        REQUESTS.with(|slot| *slot.borrow_mut() = Some(requests));
        EVENTS.with(|slot| *slot.borrow_mut() = Some(events));
        // SAFETY: the callbacks have the signatures the native side expects, and run on the main thread, as this does.
        unsafe { atelier_updater_start(relaunch_requested, update_event) }.then_some(SparkleDriver)
    }
}

impl UpdateDriver for SparkleDriver {
    fn available(&self) -> bool {
        true
    }

    fn check(&self, asked: bool) {
        // SAFETY: Sparkle was started, and this runs on the main thread.
        unsafe { atelier_updater_check(asked) }
    }

    fn install(&self) {
        // SAFETY: as in `check`.
        unsafe { atelier_updater_install() }
    }

    fn later(&self) {
        // SAFETY: as in `check`.
        unsafe { atelier_updater_later() }
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
