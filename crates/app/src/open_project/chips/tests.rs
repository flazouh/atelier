use std::collections::{HashMap, HashSet};
use beui::{PrChipData, PrState};
use super::*;
fn chip(number: u64, title: &str) -> PrChipData {
    PrChipData { number, repo: "flazouh/atelier".into(), title: title.into(), state: PrState::Open, url: format!("https://github.com/flazouh/atelier/pull/{number}").into() }
}
/// The numbers to ask about: the ones in the text that the list does not hold and nobody asked about,
/// each once.
#[test]
fn the_numbers_wanted_are_new_ones_only() {
    let texts = ["See #12 and #7, then #12 again.", "`#99` is code, not a reference; #3 is one."];
    let known: HashSet<u64> = [7].into();
    let asked: HashSet<u64> = [3].into();
    assert_eq!(wanted(texts.into_iter(), &known, &asked), [12]);
}
/// The list's chips stand first; a looked-up one fills a number the list has not, and a number that is
/// not a pull request gives no chip.
#[test]
fn the_list_wins_and_lookups_fill_in() {
    let list = vec![chip(7, "From the list")];
    let looked: HashMap<u64, Option<PrChipData>> = [(7, Some(chip(7, "Looked up"))), (12, Some(chip(12, "Twelve"))), (13, None)].into();
    let merged = merged(list, &looked);
    let titles: Vec<(u64, &str)> = merged.iter().map(|c| (c.number, c.title.as_ref())).collect();
    assert_eq!(titles, [(7, "From the list"), (12, "Twelve")]);
}
