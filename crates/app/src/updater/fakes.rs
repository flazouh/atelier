//! Fakes for the tests of the updater and of the window that serves it.
use std::{cell::Cell, rc::Rc};

use super::{RelaunchRequest, UpdateDriver};

/// How many times a driver was asked to look, to install and to wait.
#[derive(Clone, Default)]
pub struct Calls {
    checks: Rc<Cell<u32>>,
    asked: Rc<Cell<u32>>,
    installs: Rc<Cell<u32>>,
    laters: Rc<Cell<u32>>,
}

impl Calls {
    /// `(looks, looks the reader asked for, installs, waits)`.
    pub fn counts(&self) -> (u32, u32, u32, u32) {
        (self.checks.get(), self.asked.get(), self.installs.get(), self.laters.get())
    }
}

/// A driver that counts what it is asked.
pub struct Counting {
    available: bool,
    calls: Calls,
}

impl Counting {
    pub fn new(available: bool) -> (Self, Rc<Cell<u32>>) {
        let (driver, calls) = Self::recording(available);
        (driver, calls.checks)
    }

    pub fn recording(available: bool) -> (Self, Calls) {
        let calls = Calls::default();
        (Counting { available, calls: calls.clone() }, calls)
    }
}

impl UpdateDriver for Counting {
    fn available(&self) -> bool {
        self.available
    }

    fn check(&self, asked: bool) {
        self.calls.checks.set(self.calls.checks.get() + 1);
        if asked {
            self.calls.asked.set(self.calls.asked.get() + 1);
        }
    }

    fn install(&self) {
        self.calls.installs.set(self.calls.installs.get() + 1);
    }

    fn later(&self) {
        self.calls.laters.set(self.calls.laters.get() + 1);
    }
}

/// How many restarts a request was allowed, and how many it was refused.
#[derive(Clone, Default)]
pub struct Answers {
    proceeded: Rc<Cell<u32>>,
    declined: Rc<Cell<u32>>,
}

impl Answers {
    /// `(allowed, refused)`.
    pub fn counts(&self) -> (u32, u32) {
        (self.proceeded.get(), self.declined.get())
    }
}

/// A restart the updater waits on, which records the answer it got.
struct Waiting(Answers);

impl RelaunchRequest for Waiting {
    fn proceed(self: Box<Self>) {
        self.0.proceeded.set(self.0.proceeded.get() + 1);
    }

    fn decline(self: Box<Self>) {
        self.0.declined.set(self.0.declined.get() + 1);
    }
}

/// A request for a restart, and the record of its answer.
pub fn waiting() -> (Box<dyn RelaunchRequest>, Answers) {
    let answers = Answers::default();
    (Box::new(Waiting(answers.clone())), answers)
}
