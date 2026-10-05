use std::rc::Rc;

use super::super::{
    CheckOutcome, NoDriver, Question, UNAVAILABLE_NOTICE, UpdateDriver, Updater,
    fakes::{Counting, waiting},
};

#[test]
fn a_check_goes_to_the_driver_when_the_build_can_update() {
    let (driver, checks) = Counting::new(true);
    assert_eq!(Updater::new(Rc::new(driver)).check_now(), CheckOutcome::Started);
    assert_eq!(checks.get(), 1);
}

#[test]
fn a_build_that_cannot_update_never_reaches_the_driver_and_says_so() {
    let (driver, checks) = Counting::new(false);
    assert_eq!(Updater::new(Rc::new(driver)).check_now(), CheckOutcome::Unavailable);
    assert_eq!(checks.get(), 0);
    assert!(UNAVAILABLE_NOTICE.split_whitespace().count() <= 20, "one short line");
}

#[test]
fn the_updater_says_whether_its_driver_can_update() {
    assert!(Updater::new(Rc::new(Counting::new(true).0)).available());
    assert!(!Updater::new(Rc::new(Counting::new(false).0)).available());
}

#[test]
fn the_default_driver_is_the_one_that_cannot_update() {
    assert!(!NoDriver.available());
    assert_eq!(Updater::new(Rc::new(NoDriver)).check_now(), CheckOutcome::Unavailable);
}

#[test]
fn a_clean_window_proceeds_with_the_restart_at_once_and_asks_nothing() {
    let (request, answers) = waiting();
    assert!(Updater::hold(request, 0).is_none(), "nothing is held back");
    assert_eq!(answers.counts(), (1, 0));
}

#[test]
fn unsaved_edits_hold_the_restart_and_name_the_tabs_at_risk() {
    let (request, answers) = waiting();
    let held = Updater::hold(request, 2).expect("the restart waits for the reader");
    assert_eq!(held.question(), Question::about(2).unwrap());
    assert_eq!(answers.counts(), (0, 0), "neither answer is given yet");
    drop(held);
}

#[test]
fn a_yes_lets_the_held_restart_go() {
    let (request, answers) = waiting();
    Updater::hold(request, 1).unwrap().answer(true);
    assert_eq!(answers.counts(), (1, 0));
}

#[test]
fn a_no_keeps_the_app_running() {
    let (request, answers) = waiting();
    Updater::hold(request, 1).unwrap().answer(false);
    assert_eq!(answers.counts(), (0, 1));
}

#[test]
fn a_held_restart_dropped_without_an_answer_is_declined_not_lost() {
    let (request, answers) = waiting();
    drop(Updater::hold(request, 1).unwrap());
    assert_eq!(answers.counts(), (0, 1), "the updater is never left waiting");
}
