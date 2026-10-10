/// One step of a path, always absolute. The parser turns every relative step into these.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    /// A cubic curve: two control points, then the end point.
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}
