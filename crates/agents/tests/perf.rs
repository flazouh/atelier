//! The agent path's numbers, against the targets in `docs/performance.md`. Run in release, on the HP:
//! `cargo test -p atelier-agents --release --test perf -- --ignored --nocapture`
//! Each test prints its median and p95 and fails when the median misses its target.
use std::time::{Duration, Instant};

use atelier_agents::{
    claude_code::Mapper,
    session::{BlockId, Event, EventQueue},
};

const RUNS: usize = 15;

fn fixtures() -> String {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claude_code");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    names.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect()
}

/// About `bytes` of lines: the captured runs, over and over.
fn transcript(bytes: usize) -> String {
    let unit = fixtures();
    unit.repeat(bytes.div_ceil(unit.len()))
}

fn measure(mut run: impl FnMut()) -> (Duration, Duration) {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .collect();
    times.sort();
    (times[RUNS / 2], times[(RUNS * 95).div_ceil(100) - 1])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_ten_megabyte_transcript_parses_in_under_a_quarter_second() {
    let text = transcript(10 * 1024 * 1024);
    let lines = text.lines().count();
    let mut events = 0;
    let (median, p95) = measure(|| {
        let mut mapper = Mapper::new();
        let now = Instant::now();
        events = text.lines().map(|line| mapper.line(line, now).len()).sum::<usize>();
    });
    println!(
        "10 MB transcript ({lines} lines, {events} events): median {:.1} ms, p95 {:.1} ms, {:.0} MB/s",
        ms(median),
        ms(p95),
        text.len() as f64 / 1e6 / median.as_secs_f64()
    );
    assert!(median < Duration::from_millis(250), "median {:.1} ms", ms(median));
}

#[test]
#[ignore = "a measurement, run in release"]
fn one_streamed_text_line_maps_in_under_ten_microseconds() {
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"a few words of streamed text"}},"session_id":"s","parent_tool_use_id":null,"uuid":"u"}"#;
    let start_line = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"s"}"#;
    let mut mapper = Mapper::new();
    let now = Instant::now();
    mapper.line(start_line, now);
    const LINES: usize = 20_000;
    let (median, p95) = measure(|| {
        for _ in 0..LINES {
            std::hint::black_box(mapper.line(line, now));
        }
    });
    let (per_median, per_p95) = (median / LINES as u32, p95 / LINES as u32);
    println!("one text delta line: median {:.2} µs, p95 {:.2} µs", per_median.as_secs_f64() * 1e6, per_p95.as_secs_f64() * 1e6);
    assert!(per_median < Duration::from_micros(10));
}

#[test]
#[ignore = "a measurement, run in release"]
fn a_hundred_thousand_deltas_coalesce_to_one_wake_and_one_event() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    const DELTAS: usize = 100_000;
    let wakes = Arc::new(AtomicUsize::new(0));
    let (mut batches, mut count) = (0, 0);
    let (median, p95) = measure(|| {
        let counter = wakes.clone();
        let queue = EventQueue::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        for _ in 0..DELTAS {
            queue.push(Event::Text { block: BlockId(1), delta: "token ".into() });
        }
        let drained = queue.drain();
        batches += 1;
        count = drained.len();
    });
    let wakes_per_batch = wakes.load(Ordering::Relaxed) / batches;
    println!(
        "{DELTAS} deltas into the queue: median {:.1} ms, p95 {:.1} ms ({:.0} ns each), {count} event, {wakes_per_batch} wake",
        ms(median),
        ms(p95),
        median.as_nanos() as f64 / DELTAS as f64
    );
    assert_eq!((count, wakes_per_batch), (1, 1));
    assert!(median < Duration::from_millis(50));
}
