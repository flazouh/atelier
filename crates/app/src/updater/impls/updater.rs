use std::rc::Rc;

use gpui_kit::Global;

use super::super::{CheckOutcome, Held, Relaunch, RelaunchRequest, UpdateDriver, Updater};

impl Global for Updater {}

impl Updater {
    pub fn new(driver: Rc<dyn UpdateDriver>) -> Self {
        Updater { driver }
    }

    /// Whether this build can update itself.
    pub fn available(&self) -> bool {
        self.driver.available()
    }

    /// Asks the driver to look for an update, when this build can update itself.
    pub fn check_now(&self) -> CheckOutcome {
        if !self.driver.available() {
            return CheckOutcome::Unavailable;
        }
        self.driver.check();
        CheckOutcome::Started
    }

    /// Takes the updater's wait for a restart. With nothing unsaved the restart goes ahead at once and nothing is
    /// held; else the request is held until the reader answers the question that [`Held::relaunch`] words.
    pub fn hold(request: Box<dyn RelaunchRequest>, unsaved: usize) -> Option<Held> {
        match Relaunch::for_unsaved(unsaved) {
            Relaunch::Go => {
                request.proceed();
                None
            }
            relaunch => Some(Held::new(request, relaunch)),
        }
    }
}
