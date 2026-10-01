use atelier_ui::pr::PrState;

use super::*;

fn chip(repo: &str, number: u64) -> PrChipData {
    PrChipData { number, repo: repo.to_string().into(), title: "t".into(), state: PrState::Open, url: "u".into() }
}

#[test]
fn a_chip_names_a_pull_request_of_the_projects_own_repository() {
    let rows = [chip("flazouh/atelier", 3), chip("other/repo", 3), chip("flazouh/atelier", 7)];
    let numbers = |chips: Vec<PrChipData>| chips.into_iter().map(|c| (c.repo.to_string(), c.number)).collect::<Vec<_>>();
    assert_eq!(numbers(chips_of(rows.clone(), Some("flazouh/atelier"))), [("flazouh/atelier".into(), 3), ("flazouh/atelier".into(), 7)]);
    assert_eq!(chips_of(rows, None).len(), 1, "with no repository known, only the number no other holds");
}

/// With the repository not known, a number more than one repository holds names none of them: it
/// stays plain text rather than opening whichever came first.
#[test]
fn an_ambiguous_number_is_no_chip_while_the_repository_is_unknown() {
    let rows = [chip("a/one", 3), chip("b/two", 3), chip("a/one", 7)];
    let numbers: Vec<u64> = chips_of(rows, None).into_iter().map(|c| c.number).collect();
    assert_eq!(numbers, [7]);
}
