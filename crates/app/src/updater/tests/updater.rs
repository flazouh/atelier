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
fn the_updater_says_whether_its_driver_can_update() {
    let checks = Rc::new(Cell::new(0));
    assert!(Updater::new(Rc::new(Counting { available: true, checks: checks.clone() })).available());
    assert!(!Updater::new(Rc::new(Counting { available: false, checks })).available());
}

#[test]
fn the_default_driver_is_the_one_that_cannot_update() {
    assert!(!NoDriver.available());
    assert_eq!(Updater::new(Rc::new(NoDriver)).check_now(), CheckOutcome::Unavailable);
}

/// A restart request and the two counters that show which answer it got.
type Answers = (Box<Waiting>, Rc<Cell<u32>>, Rc<Cell<u32>>);

fn waiting() -> Answers {
    let (proceeded, declined) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    (Box::new(Waiting { proceeded: proceeded.clone(), declined: declined.clone() }), proceeded, declined)
}

#[test]
fn a_clean_window_proceeds_with_the_restart_at_once_and_asks_nothing() {
    let (request, proceeded, declined) = waiting();
    assert!(Updater::hold(request, 0).is_none(), "nothing is held back");
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[test]
fn unsaved_edits_hold_the_restart_and_name_the_tabs_at_risk() {
    let (request, proceeded, declined) = waiting();
    let held = Updater::hold(request, 2).expect("the restart waits for the reader");
    assert_eq!(held.relaunch().title(), "2 tabs have unsaved changes.");
    assert_eq!((proceeded.get(), declined.get()), (0, 0), "neither answer is given yet");
    drop(held);
}

#[test]
fn a_yes_lets_the_held_restart_go() {
    let (request, proceeded, declined) = waiting();
    Updater::hold(request, 1).unwrap().answer(true);
    assert_eq!((proceeded.get(), declined.get()), (1, 0));
}

#[test]
fn a_no_keeps_the_app_running() {
    let (request, proceeded, declined) = waiting();
    Updater::hold(request, 1).unwrap().answer(false);
    assert_eq!((proceeded.get(), declined.get()), (0, 1));
}

#[test]
fn a_held_restart_dropped_without_an_answer_is_declined_not_lost() {
    let (request, proceeded, declined) = waiting();
    drop(Updater::hold(request, 1).unwrap());
    assert_eq!((proceeded.get(), declined.get()), (0, 1), "the updater is never left waiting");
}
