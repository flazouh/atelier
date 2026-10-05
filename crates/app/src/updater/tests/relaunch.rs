use super::super::Relaunch;

#[test]
fn a_clean_window_lets_the_update_restart_the_app_at_once() {
    assert_eq!(Relaunch::for_unsaved(0), Relaunch::Go);
}

#[test]
fn one_unsaved_tab_asks_before_the_restart_and_says_it_in_the_singular() {
    let relaunch = Relaunch::for_unsaved(1);
    assert_eq!(relaunch, Relaunch::Ask { unsaved: 1 });
    assert_eq!(relaunch.title(), "1 tab has unsaved changes.");
}

#[test]
fn several_unsaved_tabs_ask_before_the_restart_and_count_them() {
    assert_eq!(Relaunch::for_unsaved(3).title(), "3 tabs have unsaved changes.");
}

#[test]
fn the_question_names_what_a_restart_costs_and_offers_the_way_out() {
    let relaunch = Relaunch::for_unsaved(2);
    assert_eq!(relaunch.detail(), "They are lost if you restart.");
    assert_eq!(Relaunch::BUTTONS, ["Restart Anyway", "Cancel"]);
}
