use super::super::Question;

#[test]
fn a_clean_window_has_no_question_and_the_update_restarts_the_app_at_once() {
    assert_eq!(Question::about(0), None);
}

#[test]
fn one_unsaved_tab_asks_before_the_restart_and_says_it_in_the_singular() {
    let question = Question::about(1).expect("one unsaved tab asks");
    assert_eq!(question.title(), "1 tab has unsaved changes.");
}

#[test]
fn several_unsaved_tabs_ask_before_the_restart_and_count_them() {
    assert_eq!(Question::about(3).unwrap().title(), "3 tabs have unsaved changes.");
}

#[test]
fn the_question_names_what_a_restart_costs_and_offers_the_way_out() {
    assert_eq!(Question::about(2).unwrap().detail(), "They are lost if you restart.");
    assert_eq!(Question::BUTTONS, ["Restart Anyway", "Cancel"]);
}
