//! Fakes for the tests of the updater and of the window that serves it.
use std::{cell::Cell, rc::Rc};

use super::{RelaunchRequest, UpdateDriver};

/// A driver that counts the checks it is asked for.
pub struct Counting {
    available: bool,
    checks: Rc<Cell<u32>>,
}

impl Counting {
    pub fn new(available: bool) -> (Self, Rc<Cell<u32>>) {
        let checks = Rc::new(Cell::new(0));
        (Counting { available, checks: checks.clone() }, checks)
    }
}

impl UpdateDriver for Counting {
    fn available(&self) -> bool {
        self.available
    }

    fn check(&self) {
        self.checks.set(self.checks.get() + 1);
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
