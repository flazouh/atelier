/// The rail's widest, the share of the pane it may take, the tree's width, and the least the diff card may have when the
/// tree shows.
pub const RAIL_MAX: f32 = 380.;

pub const RAIL_SHARE: f32 = 0.4;

pub const TREE: f32 = 240.;

pub const DIFF_MIN: f32 = 460.;

/// The least the diff keeps beside the rail when the reader forces the tree.
pub(super) const DIFF_FLOOR: f32 = 300.;

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

/// How many threads (and how many remarks) the rail lists at first, and each time the reader asks for more.
pub const PAGE: usize = 20;

/// A thread with more comments than this shows its first and last in the diff, and a line to show the rest.
pub const FOLD_AFTER: usize = 3;
