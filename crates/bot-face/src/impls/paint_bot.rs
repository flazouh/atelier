use gpui_kit::{Bounds, Hsla, PathBuilder, PathStyle, Pixels, Rgba, StrokeOptions, Window, px};
use lyon::tessellation::{LineCap, LineJoin};

use super::shape_trace::trace;
use crate::consts::{GROUND, VIEW};
use crate::enums::{Mood, Paint};
use crate::structs::{Affine, BotModel, Frame, Palette, Shape};

/// Draws one bot into `bounds`: the parts in order, then the cheeks, then the eyes. Call it while the window paints.
/// Return the number of shapes drawn, so a caller can count the work.
pub fn paint_bot(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    bot: &BotModel,
    frame: &Frame,
    palette: &Palette,
) -> usize {
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let side = bw.min(bh);
    let scale = side / VIEW;
    let to_px = Affine {
        a: scale,
        b: 0.0,
        c: 0.0,
        d: scale,
        e: f32::from(bounds.origin.x) + (bw - side) / 2.0,
        f: f32::from(bounds.origin.y) + (bh - side) / 2.0,
    };
    let base = to_px.then(&Affine::of_pose(&frame.root, (VIEW / 2.0, GROUND)));
    let mut drawn = 0;
    for (part, pose) in bot.parts.iter().zip(&frame.parts) {
        let m = base.then(&Affine::of_pose(pose, part.pivot));
        for s in &part.shapes {
            drawn += paint_shape(window, &m, s, palette, 1.0);
        }
    }
    if frame.cheeks > 0.003 {
        for s in &bot.cheeks {
            drawn += paint_shape(window, &base, s, palette, frame.cheeks);
        }
    }
    let looking = base.then(&Affine::translate(frame.look.0, frame.look.1));
    for mood in Mood::ALL {
        let weight = frame.eye_weights[mood.index()];
        if weight < 0.003 {
            continue;
        }
        let inner = match mood {
            Mood::Idle | Mood::Needs => {
                Affine::scale_about(1.0, frame.blink, VIEW / 2.0, bot.eye_y)
            }
            Mood::Working => Affine::translate(frame.scan_x, 0.0),
            _ => Affine::IDENTITY,
        };
        let m = looking.then(&inner);
        for s in &bot.eyes[mood.index()] {
            drawn += paint_shape(window, &m, s, palette, weight);
        }
    }
    drawn
}

fn paint_shape(window: &mut Window, m: &Affine, s: &Shape, palette: &Palette, alpha: f32) -> usize {
    let m = match s.spin {
        Some([deg, cx, cy]) => m.then(&Affine::rotate_about(deg, cx, cy)),
        None => *m,
    };
    let mut drawn = 0;
    if let Some(paint) = s.fill {
        let mut b = PathBuilder::fill();
        trace(&mut b, &m, &s.geometry);
        if let Ok(path) = b.build() {
            window.paint_path(path, colour(paint, palette, s.opacity * alpha));
            drawn += 1;
        }
    }
    if let Some(st) = s.stroke {
        let width = st.width * m.mean_scale();
        let mut options = StrokeOptions::default().with_line_width(width);
        if st.round_cap {
            options = options.with_line_cap(LineCap::Round);
        }
        if st.round_join {
            options = options.with_line_join(LineJoin::Round);
        }
        let mut b = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
        trace(&mut b, &m, &s.geometry);
        if let Ok(path) = b.build() {
            window.paint_path(path, colour(st.paint, palette, s.opacity * alpha));
            drawn += 1;
        }
    }
    drawn
}

fn colour(paint: Paint, palette: &Palette, alpha: f32) -> Hsla {
    let c: Rgba = match paint {
        Paint::Token(t) => palette.colour(t),
        Paint::Literal(c) => c,
    };
    let h: Hsla = c.into();
    h.opacity(alpha)
}
