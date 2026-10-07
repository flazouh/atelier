use std::rc::Rc;

use gpui_kit::Global;

use super::super::{CheckOutcome, Held, Question, RelaunchRequest, UpdateDriver, Updater};

impl Global for Updater {}

impl Updater {
    pub fn new(driver: Rc<dyn UpdateDriver>) -> Self {
        Updater { driver }
    }

    /// Whether this build can update itself.
    pub fn available(&self) -> bool {
        self.driver.available()
    }

    /// Asks the driver to look for an update the reader asked for, when this build can update itself.
    pub fn check_now(&self) -> CheckOutcome {
        if !self.driver.available() {
            return CheckOutcome::Unavailable;
        }
        self.driver.check(true);
        CheckOutcome::Started
    }

    /// The reader chose to restart now: the downloaded update installs.
    pub fn install(&self) {
        self.driver.install();
    }

    /// The reader chose to wait: the update installs when the app quits.
    pub fn later(&self) {
        self.driver.later();
    }

    /// Takes the updater's wait for a restart. With nothing unsaved the restart goes ahead at once and nothing is held;
    /// else the request is held until the reader answers the [`Question`].
    pub fn hold(request: Box<dyn RelaunchRequest>, unsaved: usize) -> Option<Held> {
        let Some(question) = Question::about(unsaved) else {
            request.proceed();
            return None;
        };
        Some(Held::new(request, question))
    }
}
