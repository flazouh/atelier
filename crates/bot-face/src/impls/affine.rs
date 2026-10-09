use crate::structs::{Affine, Pose};

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(x: f32, y: f32) -> Affine {
        Affine {
            e: x,
            f: y,
            ..Affine::IDENTITY
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Affine {
        Affine {
            a: sx,
            d: sy,
            ..Affine::IDENTITY
        }
    }

    /// A turn of `degrees` around the point.
    pub fn rotate_about(degrees: f32, cx: f32, cy: f32) -> Affine {
        let (s, c) = degrees.to_radians().sin_cos();
        let turn = Affine {
            a: c,
            b: s,
            c: -s,
            d: c,
            e: 0.0,
            f: 0.0,
        };
        Affine::translate(cx, cy)
            .then(&turn)
            .then(&Affine::translate(-cx, -cy))
    }

    /// A scale around the point.
    pub fn scale_about(sx: f32, sy: f32, cx: f32, cy: f32) -> Affine {
        Affine::translate(cx, cy)
            .then(&Affine::scale(sx, sy))
            .then(&Affine::translate(-cx, -cy))
    }

    /// The transform that does `other` first and `self` after it.
    pub fn then(&self, other: &Affine) -> Affine {
        Affine {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// The average size change, for a stroke width.
    pub fn mean_scale(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// A pose around a pivot: move, then turn, then scale, as the data's habits are written.
    pub fn of_pose(pose: &Pose, pivot: (f32, f32)) -> Affine {
        Affine::translate(pose.x, pose.y)
            .then(&Affine::translate(pivot.0, pivot.1))
            .then(&Affine::rotate_about(pose.r, 0.0, 0.0))
            .then(&Affine::scale(pose.sx, pose.sy))
            .then(&Affine::translate(-pivot.0, -pivot.1))
    }
}
