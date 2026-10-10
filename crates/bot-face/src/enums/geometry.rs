use super::PathCmd;

/// The outline of a shape, in view units.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    Polygon(Vec<(f32, f32)>),
    Path(Vec<PathCmd>),
}
