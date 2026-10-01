use super::*;
/// The right pane shows what the reader asked for last while it is there, and falls back to what is.
#[test]
fn the_right_pane_shows_what_was_asked_last() {
    assert_eq!(front(Front::Pulls, true, true), Front::Pulls, "the pull requests over the tasks");
    assert_eq!(front(Front::Tasks, true, true), Front::Tasks, "the tasks over the pull requests");
    assert_eq!(front(Front::Pulls, false, true), Front::Tasks, "the pull requests hidden: the tasks are back");
    assert_eq!(front(Front::Tasks, true, false), Front::Pulls, "the tasks hidden: the pull requests stay");
    assert_eq!(front(Front::Editor, false, false), Front::Editor);
}
