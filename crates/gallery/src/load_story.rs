//! The "Highlight load" story: a 10k-line Rust file in the editor and a 5k-row diff on the left, 20
//! code blocks on the right, all highlighted. With `GALLERY_SCROLL=1` it scrolls the editor, the diff
//! and the blocks each frame, logs what each frame took, how many layout nodes it built, what highlighting took inside it, and the
//! diff's layout and paint apart, prints the medians after 300 frames, and quits.
//! `LOAD_DIFF_ROWS` sets the diff's size; `GALLERY_FRAME_MS` sets the frame interval that "over" counts against
//! (8.33 ms, 120Hz, by default). It backs the frame numbers in `docs/code-editor.md` ("Performance").

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use atelier_ui::{CodeBlock, CodeEditor, DiffLine, FileDiff, FileDiffStatus, syntax::SyntaxCache};
use gpui_kit::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, Window, component::input::EditorState, div, point, px,
};

pub mod meter;
use meter::{FRAMES, Stages, Timed, report, report_count};

/// How far each frame scrolls.
const STEP: f32 = 48.;

fn rust_line(i: usize) -> [String; 6] {
    [
        format!("/// Adds {i}, and says so."),
        format!("pub fn add_{i}(value: u64) -> Result<u64, String> {{"),
        format!("    let label = \"step {i}\"; /* a comment */"),
        format!("    value.checked_add({i}).ok_or(format!(\"{{label}} overflowed\"))"),
        "}".to_string(),
        String::new(),
    ]
}

fn rust_file(lines: usize) -> String {
    (0..).flat_map(rust_line).take(lines).collect::<Vec<_>>().join("\n")
}

fn diff(rows: usize) -> String {
    let body: Vec<String> = rust_file(rows)
        .lines()
        .enumerate()
        .map(|(i, line)| match i % 12 {
            3 => format!("-{line}"),
            4 => format!("+{line} // changed"),
            _ => format!(" {line}"),
        })
        .collect();
    format!("@@ -1,{rows} +1,{rows} @@\n{}", body.join("\n"))
}

pub struct LoadStory {
    editor: Entity<EditorState>,
    diff: Vec<DiffLine>,
    blocks: Vec<String>,
    page: ScrollHandle,
    diff_scroll: UniformListScrollHandle,
    diff_times: Rc<RefCell<Stages>>,
    diff_layout: Vec<Duration>,
    diff_paint: Vec<Duration>,
    frame_times: Rc<RefCell<Stages>>,
    nodes: Vec<Duration>,
    scroll: bool,
    last: Option<Instant>,
    frames: Vec<Duration>,
    highlighting: Vec<Duration>,
}

impl LoadStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            editor: CodeEditor::state("load.rs", rust_file(10_000), window, cx),
            diff: DiffLine::parse(&diff(std::env::var("LOAD_DIFF_ROWS").ok().and_then(|n| n.parse().ok()).unwrap_or(5_000))),
            blocks: (0..20).map(|i| rust_file(25 + i)).collect(),
            page: ScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_times: Rc::default(),
            diff_layout: Vec::new(),
            diff_paint: Vec::new(),
            frame_times: Rc::default(),
            nodes: Vec::new(),
            scroll: std::env::var("GALLERY_SCROLL").is_ok_and(|v| v == "1"),
            last: None,
            frames: Vec::new(),
            highlighting: Vec::new(),
        }
    }

    /// Logs the frame that just ended, and scrolls for the next one.
    fn step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        let spent = cx.try_global::<SyntaxCache>().map(|c| c.spent).unwrap_or_default();
        if cx.has_global::<SyntaxCache>() {
            cx.global_mut::<SyntaxCache>().spent = Duration::ZERO;
        }
        if let Some(last) = self.last.replace(now) {
            if spent > Duration::from_millis(2) {
                println!("slow: frame {} spent {:.3} ms highlighting", self.frames.len(), spent.as_secs_f64() * 1000.);
            }
            self.frames.push(now - last);
            self.highlighting.push(spent);
            let stages = std::mem::take(&mut *self.diff_times.borrow_mut());
            self.diff_layout.push(stages.layout);
            self.diff_paint.push(stages.paint);
            let frame = std::mem::take(&mut *self.frame_times.borrow_mut());
            self.nodes.push(Duration::from_nanos(frame.nodes as u64));
        }
        if self.frames.len() == FRAMES {
            report("frame", &mut self.frames);
            report("highlight in frame", &mut self.highlighting);
            report("diff layout", &mut self.diff_layout);
            report("diff paint", &mut self.diff_paint);
            report_count("layout nodes", &self.nodes);
            cx.quit();
            return;
        }
        let n = self.frames.len() as f32;
        self.editor.update(cx, |e, cx| e.set_scroll_offset(point(px(0.), px(-STEP * n)), cx));
        self.page.set_offset(point(px(0.), px(-STEP * n)));
        self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.), px(-STEP * n)));
        window.request_animation_frame();
    }
}

impl Render for LoadStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.scroll {
            self.step(window, cx);
        }
        let diff = FileDiff::new("load-diff", "src/load.rs", self.diff.clone())
            .status(FileDiffStatus::Complete)
            .collapse_on_complete(false)
            .max_height(560.)
            .scroll_handle(self.diff_scroll.clone());
        let root = div()
            .size_full()
            .flex()
            .gap(px(12.))
            .p(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().flex_none().h(px(480.)).child(CodeEditor::new(&self.editor).height(px(480.))))
                    .child(Timed { child: diff.into_any_element(), stages: self.diff_times.clone() }),
            )
            .child(
                div()
                    .id("load-blocks")
                    .w(px(520.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .overflow_y_scroll()
                    .track_scroll(&self.page)
                    .children(self.blocks.iter().enumerate().map(|(i, code)| {
                        // Each at its own height: the column scrolls rather than squeezing them.
                        div().flex_none().child(
                            CodeBlock::new(("load-block", i), code.clone()).language("rust").title(format!("block_{i}.rs")),
                        )
                    })),
            );
        Timed { child: root.into_any_element(), stages: self.frame_times.clone() }
    }
}
