use std::{cell::RefCell, rc::Rc, time::Instant};

use atelier_bot_face::{BotRuntime, FaceSet, Mood, Palette, paint_bot};
use atelier_ui::scale::px;
use gpui_kit::{IntoElement, Rgba, Styled, canvas};

/// What moves a face: its runtime, and when it came on screen. A face with none stands still at its mood's pose.
pub type Motion = Option<(Rc<RefCell<BotRuntime>>, Instant)>;

/// One face, `side` design pixels square, drawn with the theme's ink for its outline. A moving face ticks its runtime
/// each frame and asks for the next one; a still face (a row, or reduce motion) asks for nothing.
pub fn face(set: Rc<FaceSet>, model: usize, mood: Mood, motion: Motion, side: f32, ink: Rgba) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |bounds, _, window, cx| {
            let bot = &set.bots[model];
            let moving = motion.as_ref().filter(|_| !cx.reduce_motion());
            let frame = match moving {
                Some((runtime, started)) => runtime.borrow_mut().tick(&set, bot, started.elapsed().as_secs_f32(), mood, None),
                None => BotRuntime::new(1, 0.).still(&set, bot, mood),
            };
            let mut palette = Palette::standard(set.body_colour(bot));
            palette.ink = ink;
            paint_bot(window, bounds, bot, &frame, &palette);
            if moving.is_some() {
                window.request_animation_frame();
            }
        },
    )
    .size(px(side))
}
