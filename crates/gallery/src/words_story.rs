//! SPARK, throwaway: "Dictation words". Five ways to let live dictation words arrive, side by side, fed the same real
//! partials (see `data.rs`) so each is judged on the same speech. The question: what is the most eloquent way for words to
//! appear while someone is still talking, when the engine hands over a whole sentence every half second, and rewrites its
//! last few words as it hears more?
//!
//! `WORDS_SPEED=0.5` slows the timeline to look closely. Nothing here is wired to the real composer.

mod data;
mod model;
mod view;

pub use view::WordsStory;
