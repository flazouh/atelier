use super::*;
use crate::RepoRef;

/// A project's pane asks only of its own repository: every search carries `repo:owner/name`. The
/// reader's whole working set asks unscoped.
#[test]
fn a_scoped_search_names_its_repository() {
    let repo = RepoRef { host: "github.com".into(), owner: "flazouh".into(), name: "lathe".into() };
    let scoped = searches(Some(&repo));
    assert_eq!(scoped.len(), SEARCHES.len());
    assert!(scoped.iter().all(|(_, search)| search.ends_with(" repo:flazouh/lathe")), "{scoped:?}");
    let whole = searches(None);
    assert!(whole.iter().all(|(_, search)| !search.contains("repo:")));
    assert_eq!(whole.iter().map(|(shelf, _)| *shelf).collect::<Vec<_>>(), SEARCHES.iter().map(|(shelf, _)| *shelf).collect::<Vec<_>>());
}
