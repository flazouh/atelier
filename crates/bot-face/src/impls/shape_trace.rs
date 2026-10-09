use gpui_kit::{PathBuilder, Pixels, Point, point, px};

use crate::consts::KAPPA;
use crate::enums::{Geometry, PathCmd};
use crate::structs::Affine;

/// Draws a shape's outline into a path builder, every point moved by `m` into pixels.
pub(crate) fn trace(b: &mut PathBuilder, m: &Affine, g: &Geometry) {
    let p = |x: f32, y: f32| -> Point<Pixels> {
        let (px_, py_) = m.apply(x, y);
        point(px(px_), px(py_))
    };
    match g {
        Geometry::Rect { x, y, w, h, r } => {
            let r = r.min(w / 2.0).min(h / 2.0);
            if r <= 0.01 {
                b.add_polygon(
                    &[p(*x, *y), p(x + w, *y), p(x + w, y + h), p(*x, y + h)],
                    true,
                );
            } else {
                let k = r * KAPPA;
                let (l, t, rt, bt) = (*x, *y, x + w, y + h);
                b.move_to(p(l + r, t));
                b.line_to(p(rt - r, t));
                b.cubic_bezier_to(p(rt, t + r), p(rt - r + k, t), p(rt, t + r - k));
                b.line_to(p(rt, bt - r));
                b.cubic_bezier_to(p(rt - r, bt), p(rt, bt - r + k), p(rt - r + k, bt));
                b.line_to(p(l + r, bt));
                b.cubic_bezier_to(p(l, bt - r), p(l + r - k, bt), p(l, bt - r + k));
                b.line_to(p(l, t + r));
                b.cubic_bezier_to(p(l + r, t), p(l, t + r - k), p(l + r - k, t));
                b.close();
            }
        }
        Geometry::Circle { cx, cy, r } => {
            let k = r * KAPPA;
            b.move_to(p(cx + r, *cy));
            b.cubic_bezier_to(p(*cx, cy + r), p(cx + r, cy + k), p(cx + k, cy + r));
            b.cubic_bezier_to(p(cx - r, *cy), p(cx - k, cy + r), p(cx - r, cy + k));
            b.cubic_bezier_to(p(*cx, cy - r), p(cx - r, cy - k), p(cx - k, cy - r));
            b.cubic_bezier_to(p(cx + r, *cy), p(cx + k, cy - r), p(cx + r, cy - k));
            b.close();
        }
        Geometry::Polygon(points) => {
            let pts: Vec<_> = points.iter().map(|(x, y)| p(*x, *y)).collect();
            b.add_polygon(&pts, true);
        }
        Geometry::Path(cmds) => {
            for c in cmds {
                match *c {
                    PathCmd::Move(x, y) => b.move_to(p(x, y)),
                    PathCmd::Line(x, y) => b.line_to(p(x, y)),
                    PathCmd::Quad(cx, cy, x, y) => b.curve_to(p(x, y), p(cx, cy)),
                    PathCmd::Close => b.close(),
                }
            }
        }
    }
}
