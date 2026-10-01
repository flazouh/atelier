//! The "Panels" story: six agent panels of two projects, side by side and in a single view, grouped by
//! project and not. `PANELS=12` opens twelve. `GALLERY_SCROLL=1` scrolls the strip sideways and prints the
//! frame numbers; with `GALLERY_SWITCH=1` it also switches the layout every 30th frame and counts those
//! frames apart.

mod helpers;
mod structs;
mod types;

pub use structs::PanelsStory;
