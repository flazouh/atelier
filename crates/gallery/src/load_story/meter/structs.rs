use std::{
    cell::RefCell,
    panic::Location,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{
    AnyElement,
    App,
    Bounds,
    Element,
    ElementId,
    GlobalElementId,
    InspectorElementId,
    IntoElement,
    LayoutId,
    Pixels,
    Window,
};

use super::helpers::node_index;

/// Time spent laying out and painting one element in a frame.
#[derive(Default)]
pub struct Stages {
    pub layout: Duration,
    pub paint: Duration,
    /// The highest layout node index seen: on a fresh tree taffy hands out indices from zero, so this is
    /// the node count when the timed element wraps the whole tree.
    pub nodes: usize,
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
