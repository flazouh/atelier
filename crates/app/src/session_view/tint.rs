//! The colour of a running subagent's mark. Colours come from a pool of seven, so two subagents that run together differ; a colour
//! goes back to the pool when its subagent ends (the subagent keeps showing it), and only when every colour is in use does one repeat (the least used).
use gpui_kit::Hsla;

use atelier_palette::{Hue, hue_at};
#[cfg(test)]
mod tests;

/// The pool, in the order it is handed out: the app's palette.
pub(super) const POOL: [Hue; 7] = Hue::ALL;
/// The index in the pool for a new subagent, given the indices the running ones hold: the first not in use, or when none is
/// free the one in use least.
pub(super) fn next(in_use: &[usize]) -> usize {
    let uses = |i: usize| in_use.iter().filter(|&&n| n == i).count();
    (0..POOL.len()).min_by_key(|&i| (uses(i), i)).unwrap_or(0)
}

pub(super) fn colour(index: usize) -> Hsla {
    hue_at(index).hsla()
}
