use super::structs::Fit;
use super::types::{
    DIFF_FLOOR, DIFF_MIN, NARROW, PADDING, Part, RAIL_GAP, RAIL_MAX, RAIL_SHARE, TREE, TREE_GAP,
};

/// The split for a pane `width` wide. From [`NARROW`] up both parts show: the rail takes at most two fifths of the pane
/// and at most [`RAIL_MAX`], and the tree shows while the diff keeps [`DIFF_MIN`] beside it, unless ⌘⇧B chose (`tree`).
/// Below it, `part` shows alone and takes the pane.
pub fn fit(width: f32, rail: bool, tree: Option<bool>, part: Part) -> Fit {
    if width < NARROW {
        return Fit { rail: (part == Part::Details).then_some(width - PADDING), tree: false, single: Some(part) };
    }
    let widest = (RAIL_SHARE * width).min(RAIL_MAX);
    let rail_space = if rail { widest + RAIL_GAP } else { 0. };
    let tree = tree.unwrap_or(width >= PADDING + rail_space + TREE + TREE_GAP + DIFF_MIN);
    let tree_space = if tree { TREE + TREE_GAP } else { 0. };
    let rail = rail.then(|| widest.min((width - PADDING - RAIL_GAP - tree_space - DIFF_FLOOR).max(200.)));
    Fit { rail, tree, single: None }
}
