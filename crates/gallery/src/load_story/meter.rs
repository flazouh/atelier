//! What the gallery's measuring stories share: how long a frame took against the display's interval, the
//! medians after a run of frames, and the layout node count of a tree. `GALLERY_SCROLL=1` runs a story's
//! scroll and prints them. `docs/performance.md` says how to read the numbers.
use std::{
    cell::RefCell,
    panic::Location,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window,
};

/// How many frames a measuring run lasts.
pub const FRAMES: usize = 300;

/// The frame interval a frame must fit: 120Hz. `GALLERY_FRAME_MS` sets another, such as 16.67 for 60Hz.
pub const FRAME_MS: f64 = 1000. / 120.;
/// The interval the report counts frames over, from `GALLERY_FRAME_MS`.
pub fn frame_limit() -> Duration {
    let ms = std::env::var("GALLERY_FRAME_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(FRAME_MS);
    Duration::from_secs_f64(ms / 1000.)
}

pub fn report(name: &str, samples: &mut [Duration]) {
    let limit = frame_limit();
    samples.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let at = |q: f64| samples[((samples.len() as f64 * q).ceil() as usize).clamp(1, samples.len()) - 1];
    let over = samples.iter().filter(|d| **d > limit).count();
    println!(
        "{name:<20} median {:>8.3} ms  p95 {:>8.3} ms  max {:>8.3} ms  over {:.2} ms: {over}/{}",
        ms(at(0.5)),
        ms(at(0.95)),
        ms(*samples.last().unwrap()),
        ms(limit),
        samples.len()
    );
}

/// Prints the node count of the first frame. Taffy reuses freed slots after that, so only the first
/// frame, built on a fresh tree, gives the exact count; the largest index of any frame comes next.
pub fn report_count(name: &str, samples: &[Duration]) {
    let count = |d: Duration| d.as_nanos();
    println!("{name:<20} first frame {:>6}  max {:>6}", count(samples[0]), count(*samples.iter().max().unwrap()));
}
/// Time spent laying out and painting one element in a frame.
#[derive(Default)]
pub struct Stages {
    pub layout: Duration,
    pub paint: Duration,
    /// The highest layout node index seen: on a fresh tree taffy hands out indices from zero, so this is
    /// the node count when the timed element wraps the whole tree.
    pub nodes: usize,
}
/// A layout id's node index, read from its debug form (`LayoutId(NodeId(n))`), for gpui keeps it private.
fn node_index(id: LayoutId) -> usize {
    let text = format!("{id:?}");
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().map_or(0, |n| (n & 0xffff_ffff) as usize)
}

/// Wraps an element and adds what its layout (request_layout, which builds a component, and
/// prepaint) and its paint take to `stages`. Paint here is building the scene on the CPU; the GPU
/// draws it later.
pub struct Timed {
    pub child: AnyElement,
    pub stages: Rc<RefCell<Stages>>,
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
        let start = Instant::now();
        let id = self.child.request_layout(window, cx);
        let mut stages = self.stages.borrow_mut();
        stages.layout += start.elapsed();
        stages.nodes = stages.nodes.max(node_index(id));
        (id, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let start = Instant::now();
        self.child.prepaint(window, cx);
        self.stages.borrow_mut().layout += start.elapsed();
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        let start = Instant::now();
        self.child.paint(window, cx);
        self.stages.borrow_mut().paint += start.elapsed();
    }
}

