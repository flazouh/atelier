//! The chips a session's text shows for `#N`: the ones the project's pull request list holds first, then
//! the ones looked up in the project's own repository with one `Forge::briefs` call for the numbers the
//! list has not. A lookup never holds a text back: the chip shows once its answer lands.
use std::collections::{HashMap, HashSet};
use atelier_ui::PrChipData;

/// The numbers in `texts` to look up: those the list does not hold (`known`) and nobody asked about
/// (`asked`), each once, in the order they first appear.
pub fn wanted<'a>(texts: impl Iterator<Item = &'a str>, known: &HashSet<u64>, asked: &HashSet<u64>) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for text in texts {
        for (_, number) in atelier_ui::pr_refs::pr_refs(text) {
            if !known.contains(&number) && !asked.contains(&number) && !out.contains(&number) {
                out.push(number);
            }
        }
    }
    out
}

/// The list's chips, then a looked-up chip for each number the list has not. `None` is an answer too:
/// that number is not a pull request, and gets no chip.
pub fn merged(list: Vec<PrChipData>, looked: &HashMap<u64, Option<PrChipData>>) -> Vec<PrChipData> {
    let held: HashSet<u64> = list.iter().map(|c| c.number).collect();
    let mut extra: Vec<PrChipData> = looked.iter().filter(|(n, _)| !held.contains(n)).filter_map(|(_, c)| c.clone()).collect();
    extra.sort_by_key(|c| c.number);
    list.into_iter().chain(extra).collect()
}

#[cfg(test)]
mod tests;
