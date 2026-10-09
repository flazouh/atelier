use std::rc::Rc;

use atelier_ui::scale::px;
use gpui_kit::{ListAlignment, ListOffset, ListState};

/// A list that lays out only the rows on screen, from the top.
pub fn new_list() -> ListState {
    ListState::new(0, ListAlignment::Top, px(160.))
}

/// Puts `next` where `current` is, and tells the list only what changed: the rows from the first that differs to the last that
/// differs. A row before the part that changes stays where it is, so a reading that finds nothing new moves nothing, and when
/// the change is above the first row in view the view is moved by as many rows as were added, so the reader keeps their place.
/// A "Load more" row after the last thread is not touched: it stands after the rows this changes.
pub fn apply<T: PartialEq>(current: &mut Rc<Vec<T>>, list: &ListState, next: Vec<T>) {
    let old = Rc::clone(current);
    let before = old.iter().zip(&next).take_while(|(a, b)| a == b).count();
    let room = old.len().min(next.len()) - before;
    let after = old
        .iter()
        .rev()
        .zip(next.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let removed = before..old.len() - after;
    let added = next.len() - before - after;
    if removed.is_empty() && added == 0 {
        return;
    }
    let place = list.logical_scroll_top();
    let (from, to) = (removed.len(), added);
    list.splice(removed.clone(), added);
    if removed.end <= place.item_ix && from != to {
        let item_ix = (place.item_ix + to).saturating_sub(from);
        list.scroll_to(ListOffset {
            item_ix,
            offset_in_item: place.offset_in_item,
        });
    }
    *current = Rc::new(next);
}

/// Shows or hides the "Load more" row after the last thread, where `rows` is how many threads the list holds.
pub fn set_tail(list: &ListState, shown: &mut bool, rows: usize, want: bool) {
    if *shown == want {
        return;
    }
    list.splice(rows..rows + usize::from(*shown), usize::from(want));
    *shown = want;
}
