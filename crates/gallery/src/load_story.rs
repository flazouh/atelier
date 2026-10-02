//! The "Highlight load" story: a 10k-line Rust file in the editor and a 5k-row diff on the left, 20
//! code blocks on the right, all highlighted. With `GALLERY_SCROLL=1` it scrolls the editor, the diff
//! and the blocks each frame, logs what each frame took, how many layout nodes it built, what highlighting took inside it, and the
//! diff's layout and paint apart, prints the medians after 300 frames, and quits.
//! `LOAD_DIFF_ROWS` sets the diff's size; `GALLERY_FRAME_MS` sets the frame interval that "over" counts against
//! (8.33 ms, 120Hz, by default). It backs the frame numbers in `docs/code-editor.md` ("Performance").

mod helpers;
pub mod meter;
mod structs;
mod types;

pub use structs::LoadStory;
