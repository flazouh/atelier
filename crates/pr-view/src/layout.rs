//! How the pull request pane splits its width. From 700 px up the rail and the diff share it: the rail takes at most
//! two fifths, the tree folds away first when the diff would get too narrow, and ⌘⇧B brings it back (the choice holds
//! until the next ⌘⇧B). Under 700 px one part shows at a time, Details or Files, as the app does in a narrow window.

/// The rail's widest, the share of the pane it may take, the tree's width, and the least the diff card may have when the
/// tree shows.
pub const RAIL_MAX: f32 = 380.;
pub const RAIL_SHARE: f32 = 0.4;
pub const TREE: f32 = 240.;
pub const DIFF_MIN: f32 = 460.;
/// The least the diff keeps beside the rail when the reader forces the tree.
const DIFF_FLOOR: f32 = 300.;
/// Below this pane width the view shows one part at a time.
pub const NARROW: f32 = 700.;
/// The pane's padding on both sides, the gap after the rail, and the gap after the tree.
pub const PADDING: f32 = 16.;
pub const RAIL_GAP: f32 = 12.;
pub const TREE_GAP: f32 = 8.;

/// The two parts of the view: the rail (description, checks, conversation, verdict, merge) and the diff with its tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Part {
    #[default]
    Details,
    Files,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The rail's width, or `None` when it is folded.
    pub rail: Option<f32>,
    pub tree: bool,
    /// In a pane under [`NARROW`], the one part that shows.
    pub single: Option<Part>,
}

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
/// How many threads (and how many remarks) the rail lists at first, and each time the reader asks for more.
pub const PAGE: usize = 20;

/// A thread with more comments than this shows its first and last in the diff, and a line to show the rest.
pub const FOLD_AFTER: usize = 3;
