use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use atelier_bot_face::{BotRuntime, FaceSet, Mood, Palette, paint_bot};
use atelier_ui::{ActiveTheme, Button, ButtonSize, ButtonVariant};
use gpui_kit::{
    Bounds, Context, InteractiveElement, IntoElement, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Point, Render, SharedString, Styled, Task, Window, bounds, canvas, div,
    point, px, size,
};

const DATA: &str = include_str!("../../../../docs/bots/faces.v1.json");
const KEEP: usize = 240;

/// What the paint closure and the event handlers share.
struct Shared {
    set: FaceSet,
    runtimes: Vec<(usize, BotRuntime)>,
    cells: Vec<Bounds<Pixels>>,
    start: Instant,
    mood: Mood,
    pointer: Option<Point<Pixels>>,
    build_ms: Vec<f32>,
    interval_ms: Vec<f32>,
    shapes: usize,
    last_frame: Option<Instant>,
    frames: u64,
    log_every: u64,
}

pub struct BotStory {
    shared: Rc<RefCell<Shared>>,
    _tick: Task<()>,
}

impl BotStory {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let set = FaceSet::from_json(DATA).expect("faces.v1.json loads");
        let env = |name: &str| std::env::var(name).ok();
        let count = env("BOTS_COUNT")
            .and_then(|v| v.parse().ok())
            .unwrap_or(30usize);
        let mood = match env("BOTS_MOOD").as_deref() {
            Some("thinking") => Mood::Thinking,
            Some("working") => Mood::Working,
            Some("done") => Mood::Done,
            Some("needs") => Mood::Needs,
            Some("stuck") => Mood::Stuck,
            _ => Mood::Idle,
        };
        let log_every = env("BOTS_LOG").and_then(|v| v.parse().ok()).unwrap_or(0u64);
        let runtimes = (0..count)
            .map(|i| {
                (
                    i % set.bots.len(),
                    BotRuntime::new(i as u32 + 1, i as f32 * 0.05),
                )
            })
            .collect();
        let shared = Rc::new(RefCell::new(Shared {
            set,
            runtimes,
            cells: Vec::new(),
            start: Instant::now(),
            mood,
            pointer: None,
            build_ms: Vec::new(),
            interval_ms: Vec::new(),
            shapes: 0,
            last_frame: None,
            frames: 0,
            log_every,
        }));
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        BotStory {
            shared,
            _tick: tick,
        }
    }
}

fn push(list: &mut Vec<f32>, v: f32) {
    list.push(v);
    if list.len() > KEEP {
        list.remove(0);
    }
}

fn average(list: &[f32]) -> f32 {
    if list.is_empty() {
        0.0
    } else {
        list.iter().sum::<f32>() / list.len() as f32
    }
}

fn percentile(list: &[f32], share: f32) -> f32 {
    if list.is_empty() {
        return 0.0;
    }
    let mut sorted = list.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted[((sorted.len() - 1) as f32 * share).round() as usize]
}

fn summary(s: &Shared) -> String {
    format!(
        "{} bots, {} shapes a frame. Build and submit: avg {:.2} ms, p99 {:.2} ms. Frame gap: avg {:.1} ms ({:.0} fps), p99 {:.1} ms.",
        s.runtimes.len(),
        s.shapes,
        average(&s.build_ms),
        percentile(&s.build_ms, 0.99),
        average(&s.interval_ms),
        1000.0 / average(&s.interval_ms).max(0.001),
        percentile(&s.interval_ms, 0.99),
    )
}

/// Lays the cells out in a grid that fills the area.
fn cells(area: Bounds<Pixels>, count: usize) -> Vec<Bounds<Pixels>> {
    let (w, h) = (f32::from(area.size.width), f32::from(area.size.height));
    let n = count.max(1) as f32;
    let cols = ((n * w / h.max(1.0)).sqrt().ceil()).max(1.0);
    let rows = (n / cols).ceil().max(1.0);
    let cell = (w / cols).min(h / rows).max(8.0);
    let (ox, oy) = (
        f32::from(area.origin.x) + (w - cell * cols) / 2.0,
        f32::from(area.origin.y),
    );
    (0..count)
        .map(|i| {
            bounds(
                point(
                    px(ox + (i as f32 % cols).floor() * cell),
                    px(oy + (i as f32 / cols).floor() * cell),
                ),
                size(px(cell), px(cell)),
            )
        })
        .collect()
}

impl Render for BotStory {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let hud = SharedString::from(summary(&self.shared.borrow()));
        let moods = Mood::ALL.iter().map(|&m| {
            Button::new(SharedString::from(format!("bot-mood-{}", m.name())))
                .label(m.name())
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .pill(true)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.shared.borrow_mut().mood = m;
                    cx.notify();
                }))
        });
        let paint_shared = self.shared.clone();
        let stage = canvas(
            |area, _, _| area,
            move |area, _, window, cx| {
                let still = cx.reduce_motion();
                let started = Instant::now();
                let mut guard = paint_shared.borrow_mut();
                let s = &mut *guard;
                let t = s.start.elapsed().as_secs_f32();
                s.cells = cells(area, s.runtimes.len());
                let mut drawn = 0;
                for (i, (bot_ix, runtime)) in s.runtimes.iter_mut().enumerate() {
                    let cell = s.cells[i];
                    let bot = &s.set.bots[*bot_ix];
                    let look = s.pointer.map(|p| {
                        let c = cell.center();
                        let reach = f32::from(cell.size.width) * 3.0;
                        (
                            ((f32::from(p.x) - f32::from(c.x)) / reach).clamp(-1.0, 1.0),
                            ((f32::from(p.y) - f32::from(c.y)) / reach).clamp(-1.0, 1.0),
                        )
                    });
                    let frame = if still {
                        runtime.still(&s.set, bot, s.mood)
                    } else {
                        runtime.tick(&s.set, bot, t, s.mood, look)
                    };
                    let palette = Palette::standard(s.set.body_colour(bot));
                    drawn += paint_bot(window, cell, bot, &frame, &palette);
                }
                s.shapes = drawn;
                push(&mut s.build_ms, started.elapsed().as_secs_f32() * 1000.0);
                if let Some(last) = s.last_frame {
                    push(
                        &mut s.interval_ms,
                        started.duration_since(last).as_secs_f32() * 1000.0,
                    );
                }
                s.last_frame = Some(started);
                s.frames += 1;
                if s.log_every > 0 && s.frames.is_multiple_of(s.log_every) {
                    eprintln!("bots: {}", summary(s));
                }
                if !still {
                    window.request_animation_frame();
                }
            },
        )
        .size_full();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(16.))
            .child(div().flex().gap(px(8.)).children(moods))
            .child(div().text_size(px(12.)).text_color(muted).child(hud))
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| {
                        this.shared.borrow_mut().pointer = Some(e.position);
                    }))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, e: &MouseDownEvent, _, _| {
                            let mut guard = this.shared.borrow_mut();
                            let s = &mut *guard;
                            let t = s.start.elapsed().as_secs_f32();
                            if let Some(i) = s.cells.iter().position(|c| c.contains(&e.position)) {
                                s.runtimes[i].1.react(t);
                            }
                        }),
                    )
                    .child(stage),
            )
    }
}
