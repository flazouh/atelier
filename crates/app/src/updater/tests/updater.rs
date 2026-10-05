use std::{cell::Cell, rc::Rc};

use super::super::{CheckOutcome, NoDriver, RelaunchRequest, UNAVAILABLE_NOTICE, UpdateDriver, Updater};

/// A driver that counts the checks it is asked for.
struct Counting {
    available: bool,
    checks: Rc<Cell<u32>>,
}

impl UpdateDriver for Counting {
    fn available(&self) -> bool {
        self.available
    }

    fn check(&self) {
        self.checks.set(self.checks.get() + 1);
    }
}

/// A restart the updater waits on, which records the answer it got.
struct Waiting {
    proceeded: Rc<Cell<u32>>,
    declined: Rc<Cell<u32>>,
}

impl RelaunchRequest for Waiting {
    fn proceed(self: Box<Self>) {
        self.proceeded.set(self.proceeded.get() + 1);
    }

    fn decline(self: Box<Self>) {
        self.declined.set(self.declined.get() + 1);
    }
}

#[test]
fn a_check_goes_to_the_driver_when_the_build_can_update() {
    let checks = Rc::new(Cell::new(0));
    let updater = Updater::new(Rc::new(Counting { available: true, checks: checks.clone() }));
    assert_eq!(updater.check_now(), CheckOutcome::Started);
    assert_eq!(checks.get(), 1);
}

#[test]
fn a_build_that_cannot_update_never_reaches_the_driver_and_says_so() {
    let checks = Rc::new(Cell::new(0));
    let updater = Updater::new(Rc::new(Counting { available: false, checks: checks.clone() }));
    assert_eq!(updater.check_now(), CheckOutcome::Unavailable);
    assert_eq!(checks.get(), 0);
    assert!(UNAVAILABLE_NOTICE.split_whitespace().count() <= 20, "one short line");
}

#[test]
fn the_default_driver_is_the_one_that_cannot_update() {
    assert!(!NoDriver.available());
    assert_eq!(Updater::new(Rc::new(NoDriver)).check_now(), CheckOutcome::Unavailable);
}

#[test]
fn a_clean_window_proceeds_with_the_restart_and_declines_nothing() {
    let (proceeded, declined) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let request = Waiting { proceeded: proceeded.clone(), declined: declined.clone() };
    Updater::answer(Box::new(request), 0, |_| unreachable!("a clean window is not asked"));
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[test]
fn unsaved_edits_hold_the_restart_until_the_reader_says_yes() {
    let (proceeded, declined) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let request = Waiting { proceeded: proceeded.clone(), declined: declined.clone() };
    let mut asked = None;
    Updater::answer(Box::new(request), 2, |relaunch| {
        asked = Some(relaunch);
        true
    });
    assert_eq!(asked.map(|r| r.title()), Some("2 tabs have unsaved changes.".to_string()));
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[test]
fn unsaved_edits_keep_the_app_running_when_the_reader_says_no() {
    let (proceeded, declined) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let request = Waiting { proceeded: proceeded.clone(), declined: declined.clone() };
    Updater::answer(Box::new(request), 1, |_| false);
    assert_eq!((proceeded.get(), declined.get()), (0, 1));
}
