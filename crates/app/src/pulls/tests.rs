use beui::pr::PrState;

use super::*;

fn chip(repo: &str, number: u64) -> PrChipData {
    PrChipData { number, repo: repo.to_string().into(), title: "t".into(), state: PrState::Open, url: "u".into() }
}

#[test]
fn a_chip_names_a_pull_request_of_the_projects_own_repository() {
    let rows = [chip("flazouh/lathe", 3), chip("other/repo", 3), chip("flazouh/lathe", 7)];
    let numbers = |chips: Vec<PrChipData>| chips.into_iter().map(|c| (c.repo.to_string(), c.number)).collect::<Vec<_>>();
    assert_eq!(numbers(chips_of(rows.clone(), Some("flazouh/lathe"))), [("flazouh/lathe".into(), 3), ("flazouh/lathe".into(), 7)]);
    assert_eq!(chips_of(rows, None).len(), 3, "with no repository known, every one");
}
