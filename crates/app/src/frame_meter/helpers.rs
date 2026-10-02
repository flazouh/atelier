use std::{cell::RefCell, rc::Rc, time::Duration};

use super::structs::Meter;
use super::types::Mode;

thread_local! {
    /// The last frame's layout and paint, for a timing that wants its own frame's cost.
    pub(super) static LAST: std::cell::Cell<Duration> = const { std::cell::Cell::new(Duration::ZERO) };
}

/// The last frame's layout and paint on the CPU; zero without `ATELIER_FRAMES=1`.
pub fn last_frame() -> Duration {
    LAST.with(std::cell::Cell::get)
}

pub fn enabled() -> bool {
    mode(std::env::var("ATELIER_FRAMES").ok().as_deref()) != Mode::Off
}

pub fn mode(value: Option<&str>) -> Mode {
    match value {
        Some("1") => Mode::Reports,
        Some("each") => Mode::Each,
        _ => Mode::Off,
    }
}

pub(super) fn each() -> bool {
    static EACH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *EACH.get_or_init(|| mode(std::env::var("ATELIER_FRAMES").ok().as_deref()) == Mode::Each)
}

/// Adds `spent` to this frame's part `name`, for time spent outside any [`Part`](super::structs::Part) (a view's own render).
pub fn add_part(meter: &Rc<RefCell<Meter>>, name: &'static str, spent: Duration) {
    meter.borrow_mut().add_part(name, spent);
}

/// One frame's line under `ATELIER_FRAMES=each`: when, the whole frame, then each part, in ms.
pub fn each_line(at_ms: u128, frame: Duration, parts: &[(&'static str, Duration)]) -> String {
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let mut line = format!("frame {at_ms} {:.2}", ms(frame));
    for (name, spent) in parts {
        line.push_str(&format!(" {name}={:.2}", ms(*spent)));
    }
    line
}
