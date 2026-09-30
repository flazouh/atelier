use super::*;
/// The right pane shows what the reader asked for last while it is there, and falls back to what is.
#[test]
fn the_right_pane_shows_what_was_asked_last() {
    assert_eq!(front(Front::Pulls, true, true, false), Front::Pulls, "the pull requests over an open review");
    assert_eq!(front(Front::Review, true, true, false), Front::Review, "a review opened over them");
    assert_eq!(front(Front::Pulls, true, false, false), Front::Review, "the pull requests hidden: the review is back");
    assert_eq!(front(Front::Review, false, true, false), Front::Pulls, "the review closed: the pull requests stay");
    assert_eq!(front(Front::Editor, false, false, false), Front::Editor);
    assert_eq!(front(Front::Tasks, true, true, true), Front::Tasks, "the tasks over a review and the pull requests");
    assert_eq!(front(Front::Tasks, true, false, false), Front::Review, "the tasks hidden: the review is back");
    assert_eq!(front(Front::Editor, false, false, true), Front::Tasks, "nothing else open: the tasks stay");
}
