//! The conversation's scroll, as beui's message scroller: while the reader is at the live edge it glides to each new end
//! of the output, a jump to a message glides until that message is in the middle of the view, and the reader's own
//! scroll lets go of both. Under Reduce Motion it jumps.
//!
//! The list's own tail follow snaps to the end inside its layout, so the follow is kept here instead: after the list has
//! laid out (see [`Glide::tick`]), the scroll moves part of the way to where it should be, and asks for the next frame.
use std::{cell::RefCell, rc::Rc, time::Instant};

use gpui_kit::{ListOffset, ListState, Window, px};

/// How near the end the reader must stop for the conversation to keep following (beui's `followThreshold`).
pub const FOLLOW_REACH: f32 = 56.;

/// How fast a glide closes the gap: it covers 98% of it in about four of these, beui's 320ms smooth scroll.
const TAU: f32 = 0.08;

/// Where the scroll goes from `current` toward `target` in `dt` seconds.
pub fn step(current: f32, target: f32, dt: f32) -> f32 {
    let left = target - current;
    if left.abs() < 0.5 {
        return target;
    }
    current + left * (1. - (-dt / TAU).exp())
}

/// Where the scroll must be for a message `top` px down the list and `height` tall to sit in the middle of a `view` px
/// view, inside `0..=max`.
pub fn centred(top: f32, height: f32, view: f32, max: f32) -> f32 {
    (top - (view - height) / 2.).clamp(0., max)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Goal {
    End,
    Row(usize),
}

struct State {
    following: bool,
    goal: Option<Goal>,
    last: Option<Instant>,
    max: f32,
    scrolled: bool,
}

/// One conversation's follow and glide. Clones share it.
#[derive(Clone)]
pub struct Glide(Rc<RefCell<State>>);

impl Default for Glide {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(State { following: true, goal: None, last: None, max: 0., scrolled: false })))
    }
}

impl Glide {
    /// Whether the conversation follows the output.
    pub fn following(&self) -> bool {
        self.0.borrow().following
    }

    /// Takes hold of the end and glides there.
    pub fn follow(&self) {
        let mut s = self.0.borrow_mut();
        s.following = true;
        s.goal = Some(Goal::End);
    }

    /// Lets go of the end and glides until `row` is in the middle of the view.
    pub fn go_to(&self, row: usize) {
        let mut s = self.0.borrow_mut();
        s.following = false;
        s.goal = Some(Goal::Row(row));
    }

    /// The reader scrolled: the next tick decides from where they stopped whether the conversation still follows.
    pub fn scrolled(&self) {
        self.0.borrow_mut().scrolled = true;
    }

    /// Moves the scroll one frame on; called once the list has laid out.
    pub fn tick(&self, list: &ListState, reduce_motion: bool, window: &mut Window) {
        let max = f32::from(list.max_offset_for_scrollbar().y);
        let pinned = list.logical_scroll_top().item_ix >= list.item_count();
        let current = if pinned { max } else { (-f32::from(list.scroll_px_offset_for_scrollbar().y)).min(max) };
        let mut s = self.0.borrow_mut();
        if std::mem::take(&mut s.scrolled) {
            s.goal = None;
            s.following = max - current <= FOLLOW_REACH;
        }
        if s.following && s.goal.is_none() && max > s.max + 0.5 {
            s.goal = Some(Goal::End);
        }
        s.max = max;
        // A list held at its end snaps to each new end; held at a pixel it leaves the new output below, to glide to.
        // Under Reduce Motion the snap is what is wanted.
        if s.following && pinned && !reduce_motion {
            scroll_at(list, max);
        }
        let target = match s.goal {
            None => {
                s.last = None;
                return;
            }
            Some(Goal::End) => max,
            Some(Goal::Row(row)) => {
                list.scroll_to(ListOffset { item_ix: row, offset_in_item: px(0.) });
                let top = -f32::from(list.scroll_px_offset_for_scrollbar().y);
                let height = list.bounds_for_item(row).map_or(0., |b| f32::from(b.size.height));
                centred(top, height, f32::from(list.viewport_bounds().size.height), max)
            }
        };
        // It ends on a frame that starts where it should be: rows measured on the way can move the place it aims for.
        if (target - current).abs() < 0.5 {
            scroll_at(list, current);
            s.goal = None;
            s.last = None;
            return;
        }
        let now = Instant::now();
        let dt = s.last.map_or(1. / 60., |at| now.duration_since(at).as_secs_f32()).min(0.064);
        scroll_at(list, if reduce_motion { target } else { step(current, target, dt) });
        s.last = Some(now);
        window.request_animation_frame();
    }
}

/// Puts the list's top `y` px down its rows.
fn scroll_at(list: &ListState, y: f32) {
    list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
    list.scroll_by(px(y));
}

#[cfg(test)]
mod tests;
