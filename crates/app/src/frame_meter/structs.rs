use std::{
    cell::RefCell,
    panic::Location,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

use super::types::{FRAMES, LIMIT};
use super::helpers::{LAST, each, each_line};

#[derive(Default)]
pub struct Meter {
    pub(super) current: Duration,
    /// This frame's layout (request_layout and prepaint), and every frame's.
    pub(super) layout: Duration,
    pub(super) layouts: Vec<Duration>,
    pub(super) frames: Vec<Duration>,
    /// This frame's named parts ([`Part`]), in the order they were drawn.
    pub(super) parts: Vec<(&'static str, Duration)>,
}

impl Meter {
    pub(super) fn add_part(&mut self, name: &'static str, spent: Duration) {
        match self.parts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, total)) => *total += spent,
            None => self.parts.push((name, spent)),
        }
    }
}

impl Meter {
    pub(super) fn frame_done(&mut self) {
        LAST.with(|last| last.set(self.current));
        let parts = std::mem::take(&mut self.parts);
        if each() {
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            eprintln!("{}", each_line(at.as_millis(), self.current, &parts));
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

/// Times one part of the window (the sidebar, the panels) inside the frame [`Timed`] times, so a frame's line
/// says where its time went. A part drawn twice in a frame adds up.
pub struct Part {
    pub name: &'static str,
    pub child: AnyElement,
    pub meter: Rc<RefCell<Meter>>,
}

impl IntoElement for Part {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Part {
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
        self.meter.borrow_mut().add_part(self.name, at.elapsed());
        (id, ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let at = Instant::now();
        self.child.prepaint(window, cx);
        self.meter.borrow_mut().add_part(self.name, at.elapsed());
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        let at = Instant::now();
        self.child.paint(window, cx);
        self.meter.borrow_mut().add_part(self.name, at.elapsed());
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
