//! The colour of a running subagent's mark. Colours come from a pool of seven, so two subagents that run together differ; a colour
//! goes back to the pool when its subagent ends (the subagent keeps showing it), and only when every colour is in use does one repeat (the least used).
use gpui_kit::Hsla;
#[cfg(test)]
mod tests;

/// The pool, in the order it is handed out.
pub(super) const POOL: [u32; 7] = [0xFF5C59, 0xFF8D22, 0xFBD73C, 0x15DB95, 0x4ACFFF, 0x9758FF, 0xFF78F7];

/// The index in the pool for a new subagent, given the indices the running ones hold: the first not in use, or when none is
/// free the one in use least.
pub(super) fn next(in_use: &[usize]) -> usize {
    let uses = |i: usize| in_use.iter().filter(|&&n| n == i).count();
    (0..POOL.len()).min_by_key(|&i| (uses(i), i)).unwrap_or(0)
}

pub(super) fn colour(index: usize) -> Hsla {
    gpui_kit::rgb(POOL[index % POOL.len()]).into()
}
