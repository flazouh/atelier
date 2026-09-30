//! `LATHE_FRAMES=1`: how long each frame's layout and paint take on the CPU, over the whole window,
//! reported every 300 frames as a median, a p95, a worst and a count over 8 ms (120 Hz). The window's
//! root is wrapped in [`Timed`]; paint here is building the scene, which the GPU draws later.
//! `LATHE_FRAMES=each` also prints one line per frame, `frame <unix ms> <cpu ms>`, to count the frames of an
//! idle screen or to take one animation's median and p95.

use std::{
    cell::RefCell,
    panic::Location,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window};

/// Frames a report covers.
const FRAMES: usize = 300;
/// A frame must fit this: 120 Hz.
const LIMIT: Duration = Duration::from_micros(8_333);

thread_local! {
    /// The last frame's layout and paint, for a timing that wants its own frame's cost.
    static LAST: std::cell::Cell<Duration> = const { std::cell::Cell::new(Duration::ZERO) };
}

/// The last frame's layout and paint on the CPU; zero without `LATHE_FRAMES=1`.
pub fn last_frame() -> Duration {
    LAST.with(std::cell::Cell::get)
}

pub fn enabled() -> bool {
    mode(std::env::var("LATHE_FRAMES").ok().as_deref()) != Mode::Off
}

/// What `LATHE_FRAMES` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    /// A report every 300 frames.
    Reports,
    /// The reports, and a line per frame.
    Each,
}

pub fn mode(value: Option<&str>) -> Mode {
    match value {
        Some("1") => Mode::Reports,
        Some("each") => Mode::Each,
        _ => Mode::Off,
    }
}

fn each() -> bool {
    static EACH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *EACH.get_or_init(|| mode(std::env::var("LATHE_FRAMES").ok().as_deref()) == Mode::Each)
}

#[derive(Default)]
pub struct Meter {
    current: Duration,
    /// This frame's layout (request_layout and prepaint), and every frame's.
    layout: Duration,
    layouts: Vec<Duration>,
    frames: Vec<Duration>,
}

impl Meter {
    fn frame_done(&mut self) {
        LAST.with(|last| last.set(self.current));
        if each() {
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            eprintln!("frame {} {:.2}", at.as_millis(), self.current.as_secs_f64() * 1000.);
        }
        self.frames.push(std::mem::take(&mut self.current));
        self.layouts.push(std::mem::take(&mut self.layout));
        if self.frames.len() >= FRAMES {
            let mut f = std::mem::take(&mut self.frames);
            let mut l = std::mem::take(&mut self.layouts);
            f.sort();
            l.sort();
            let ms = |d: Duration| d.as_secs_f64() * 1000.;
            eprintln!(
                "frames: median {:.2} ms, p95 {:.2} ms, worst {:.2} ms, over 8.3 ms: {}/{}; layout median {:.2} ms, p95 {:.2} ms",
                ms(f[f.len() / 2]),
                ms(f[f.len() * 95 / 100]),
                ms(f[f.len() - 1]),
                f.iter().filter(|d| **d > LIMIT).count(),
                f.len(),
                ms(l[l.len() / 2]),
                ms(l[l.len() * 95 / 100]),
            );
        }
    }
}

/// Wraps the window's root and times its layout and paint.
pub struct Timed {
    pub child: AnyElement,
    pub meter: Rc<RefCell<Meter>>,
}

impl IntoElement for Timed {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Timed {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let at = Instant::now();
        let id = self.child.request_layout(window, cx);
        let mut meter = self.meter.borrow_mut();
        meter.current += at.elapsed();
        meter.layout += at.elapsed();
        (id, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let at = Instant::now();
        self.child.prepaint(window, cx);
        let mut meter = self.meter.borrow_mut();
        meter.current += at.elapsed();
        meter.layout += at.elapsed();
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        let at = Instant::now();
        self.child.paint(window, cx);
        let mut meter = self.meter.borrow_mut();
        meter.current += at.elapsed();
        meter.frame_done();
    }
}

#[cfg(test)]
mod tests;
