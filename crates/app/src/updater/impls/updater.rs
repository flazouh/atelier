use std::rc::Rc;

use gpui_kit::Global;

use super::super::{CheckOutcome, Relaunch, RelaunchRequest, UpdateDriver, Updater};

impl Global for Updater {}

impl Updater {
    pub fn new(driver: Rc<dyn UpdateDriver>) -> Self {
        Updater { driver }
    }

    /// Asks the driver to look for an update, when this build can update itself.
    pub fn check_now(&self) -> CheckOutcome {
        if !self.driver.available() {
            return CheckOutcome::Unavailable;
        }
        self.driver.check();
        CheckOutcome::Started
    }

    /// Answers the updater's wait for a restart. With nothing unsaved it proceeds at once; else `ask` puts the
    /// question to the reader, and the restart goes ahead only on a yes.
    pub fn answer(request: Box<dyn RelaunchRequest>, unsaved: usize, ask: impl FnOnce(Relaunch) -> bool) {
        match Relaunch::for_unsaved(unsaved) {
            Relaunch::Go => request.proceed(),
            relaunch if ask(relaunch) => request.proceed(),
            _ => request.decline(),
        }
    }
}
