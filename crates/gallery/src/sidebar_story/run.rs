//! A run of frames for the stories of the sidebar and the panels, measured with `crate::load_story::meter`.
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::AnyElement;

use crate::load_story::meter::{FRAMES, Stages, Timed, report, report_count};

/// A run of frames: the gap between one frame's start and the next, the layout and paint time of the
/// timed tree, and its node count. A story calls [`Run::frame`] at the top of each render, does what the
/// run asks (scroll, switch), and wraps its root with [`Run::wrap`]. After [`FRAMES`] frames the run prints
/// and says to quit.
pub struct Run {
    last: Option<Instant>,
    frames: Vec<Duration>,
    stages: Rc<RefCell<Stages>>,
    layout: Vec<Duration>,
    paint: Vec<Duration>,
    nodes: Vec<Duration>,
    /// Frames in which the story marked a switch, apart from the rest.
    switched: Vec<Duration>,
    mark_switch: bool,
}

impl Run {
    pub fn new() -> Self {
        Self {
            last: None,
            frames: Vec::new(),
            stages: Rc::default(),
            layout: Vec::new(),
            paint: Vec::new(),
            nodes: Vec::new(),
            switched: Vec::new(),
            mark_switch: false,
        }
    }

    /// Logs the frame that just ended. Gives the index of the frame to draw next, or `None` when the run
    /// is over and has printed.
    pub fn frame(&mut self) -> Option<usize> {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            let took = now - last;
            if std::mem::take(&mut self.mark_switch) {
                self.switched.push(took);
            } else {
                self.frames.push(took);
            }
            let stages = std::mem::take(&mut *self.stages.borrow_mut());
            self.layout.push(stages.layout);
            self.paint.push(stages.paint);
            self.nodes.push(Duration::from_nanos(stages.nodes as u64));
        }
        let done = self.frames.len() + self.switched.len();
        if done >= FRAMES {
            report("frame", &mut self.frames);
            if !self.switched.is_empty() {
                report("frame with a switch", &mut self.switched);
            }
            report("layout and prepaint", &mut self.layout);
            report("paint", &mut self.paint);
            report_count("layout nodes", &self.nodes);
            return None;
        }
        Some(done)
    }

    /// The frame being drawn now has switched something, so it is counted apart.
    pub fn switched(&mut self) {
        self.mark_switch = true;
    }

    /// Wraps the story's root so its layout and paint are timed.
    pub fn wrap(&self, root: AnyElement) -> Timed {
        Timed { child: root, stages: self.stages.clone() }
    }
}

impl Default for Run {
    fn default() -> Self {
        Self::new()
    }
}
