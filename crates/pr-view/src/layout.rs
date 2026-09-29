//! How the pull request pane splits its width. It fits any pane from about 1100 px up: the rail and the
//! right pane share the width, the tree folds away first when the diff would get too narrow, then the rail
//! narrows. ⌘⇧B brings the tree back, and the choice holds until the next ⌘⇧B.

/// The rail's widest and narrowest, the tree's width, and the least the diff card may have.
pub const RAIL_MAX: f32 = 380.;
pub const RAIL_MIN: f32 = 300.;
pub const TREE: f32 = 240.;
pub const DIFF_MIN: f32 = 460.;
/// The pane's padding on both sides, the gap after the rail, and the gap after the tree.
pub const PADDING: f32 = 16.;
pub const RAIL_GAP: f32 = 12.;
pub const TREE_GAP: f32 = 8.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The rail's width, or `None` when it is folded.
    pub rail: Option<f32>,
    pub tree: bool,
}

/// The split for a pane `width` wide: the tree shows while the diff keeps [`DIFF_MIN`] beside a full
/// rail, unless ⌘⇧B chose (`tree`); then the rail takes what the diff leaves, from [`RAIL_MAX`] down to
/// [`RAIL_MIN`].
pub fn fit(width: f32, rail: bool, tree: Option<bool>) -> Fit {
    let rail_space = if rail { RAIL_MAX + RAIL_GAP } else { 0. };
    let tree = tree.unwrap_or(width >= PADDING + rail_space + TREE + TREE_GAP + DIFF_MIN);
    let tree_space = if tree { TREE + TREE_GAP } else { 0. };
    let rail = rail.then(|| (width - PADDING - RAIL_GAP - tree_space - DIFF_MIN).clamp(RAIL_MIN, RAIL_MAX));
    Fit { rail, tree }
}
