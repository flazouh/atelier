use std::{
    collections::{HashMap},
    time::{Duration, Instant},
};

use atelier_ui::ChangedFile;
use gpui_kit::{Entity, FocusHandle, Window, component::input::EditorState};
use atelier_review::FileReview;

use crate::{
    review_state::Decided,
    review_text::{moved_caret, splice},
    };
use super::structs::PaneFile;
use super::types::Scope;

/// Focuses `handle` now and again at each of the next `frames` frames: a composer in a row block is
/// painted only once the editor has laid it out, a frame or two later, and a frame that does not paint
/// the focused handle drops the focus. It counts as focused until then, so the retry cannot ask.
pub(super) fn focus_once_painted(handle: FocusHandle, frames: usize, window: &mut Window, cx: &mut gpui_kit::App) {
    handle.focus(window, cx);
    if frames > 0 {
        window.on_next_frame(move |window, cx| focus_once_painted(handle, frames - 1, window, cx));
    }
}

/// With `ATELIER_TIMINGS=1`, prints what an action cost: its own `work`, how long after `since` the frame
/// that shows it began (the wait for the display), and that frame's layout and paint (with
/// `ATELIER_FRAMES=1`). A next-frame callback runs as a frame begins, before it is drawn, so the frame's
/// cost is read at the start of the one after it.
pub(super) fn timing(window: &Window, what: String, since: Instant, work: Duration) {
    if std::env::var("ATELIER_TIMINGS").is_ok_and(|v| v == "1") {
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        window.on_next_frame(move |window, _| {
            let began = since.elapsed();
            window.on_next_frame(move |_, _| {
                let frame = crate::frame_meter::last_frame();
                eprintln!("{what}: work {:.1} ms, its frame began after {:.1} ms and took {:.1} ms", ms(work), ms(began), ms(frame));
            });
        });
    }
}

/// The list the tree and the walk take: each file's path, `+a -r` and change.
pub(super) fn changed_of(files: &[PaneFile]) -> Vec<ChangedFile> {
    let reviews: Vec<FileReview> = files.iter().map(|f| f.review.clone()).collect();
    atelier_review::present::changed_files(&reviews)
}

/// The files of `scope`, with what the reader decided before where the session kept it.
pub(super) fn files_of(reviews: Vec<FileReview>, scope: Scope, decided: &Decided, committed: &HashMap<(Scope, String), String>) -> Vec<PaneFile> {
    reviews
        .into_iter()
        .map(|review| {
            let mut file = PaneFile::new(review);
            if let Some((merged, on_disk)) = decided.get(&(scope, file.review.path.clone())) {
                (file.merged, file.on_disk) = (merged.clone(), on_disk.clone());
            }
            file.committed = committed.get(&(scope, file.review.path.clone())).cloned();
            file
        })
        .collect()
}

/// Puts `text` in the editor as the one edit between its text and `text`, keeping the caret with the
/// text around it and the scroll where it was.
pub(super) fn put_text(editor: &Entity<EditorState>, text: &str, window: &mut Window, cx: &mut gpui_kit::App) {
    editor.update(cx, |state, cx| {
        let old = state.value().to_string();
        let (range, with) = splice(&old, text);
        if range.is_empty() && with.is_empty() {
            return;
        }
        let (caret, scroll) = (state.cursor(), state.scroll_offset());
        state.set_selected_range(range.clone(), cx);
        state.replace(with.to_string(), window, cx);
        let caret = moved_caret(caret, &range, with.len());
        state.set_selected_range(caret..caret, cx);
        state.set_scroll_offset(scroll, cx);
    });
}
