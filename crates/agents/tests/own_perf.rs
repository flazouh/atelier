//! The own agent's numbers. Run in release, on the HP:
//! `cargo test -p atelier-agents --release --test own_perf -- --ignored --nocapture --test-threads=1`
//! Each test prints its median and p95 and fails when the median misses its target.
use std::{
    io,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use atelier_agents::{
    own::{
        Block, Cancel, Delta, Message, Model, ModelError, ModelRequest, OwnAgent, OwnOptions, Reply, StopReason, TokenUsage,
        anthropic::StreamState,
        sse::Parser,
    },
    session::{Backend, Command, Event, OpenRequest, PermissionMode},
};
use atelier_project::{ChangeSink, Command as Spawn, Entry, GitOutput, LocalProject, Match, Process, Project, Query, Watch};
use serde_json::json;

const RUNS: usize = 15;

fn measure(mut run: impl FnMut() -> Duration) -> (Duration, Duration) {
    let mut times: Vec<Duration> = (0..RUNS).map(|_| run()).collect();
    times.sort();
    (times[RUNS / 2], times[(RUNS * 95).div_ceil(100) - 1])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000_000.
}

/// About `bytes` of a Messages API stream: text deltas of a word each, the shape of a long answer.
fn stream(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes + 1024);
    let event = |name: &str, data: serde_json::Value| format!("event: {name}\ndata: {data}\n\n");
    out.extend(event("message_start", json!({"type": "message_start", "message": {"usage": {"input_tokens": 10, "output_tokens": 1}}})).into_bytes());
    out.extend(event("content_block_start", json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}})).into_bytes());
    let delta = event("content_block_delta", json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "streaming "}}));
    while out.len() < bytes {
        out.extend(delta.as_bytes());
    }
    out.extend(event("content_block_stop", json!({"type": "content_block_stop", "index": 0})).into_bytes());
    out.extend(event("message_delta", json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 999}})).into_bytes());
    out.extend(event("message_stop", json!({"type": "message_stop"})).into_bytes());
    out
}

fn parse(bytes: &[u8]) -> (Duration, usize) {
    let start = Instant::now();
    let mut parser = Parser::default();
    let mut state = StreamState::default();
    let mut events = 0usize;
    let mut text = 0usize;
    for chunk in bytes.chunks(16 * 1024) {
        let mut batch = Vec::new();
        parser.feed(chunk, &mut |e| batch.push(e)).unwrap();
        for event in batch {
            events += 1;
            state.on_event(event.name.as_deref(), &event.data, &mut |d| {
                if let Delta::Text(t) = d {
                    text += t.len();
                }
            })
            .unwrap();
        }
    }
    let reply = state.finish().unwrap();
    assert!(matches!(&reply.blocks[0], Block::Text { text: t } if t.len() == text));
    (start.elapsed(), events)
}

#[test]
#[ignore = "a measuring run; needs --release"]
fn the_stream_parses_faster_than_any_model_can_talk() {
    let bytes = stream(10 * 1024 * 1024);
    let (mut events, mut took) = (0, Duration::ZERO);
    let (median, p95) = measure(|| {
        let (t, n) = parse(&bytes);
        events = n;
        took = t;
        t
    });
    let mb = bytes.len() as f64 / 1_048_576.;
    println!(
        "parse a {mb:.1} MB stream ({events} events, SSE + JSON + reply): median {:.1} ms ({:.0} MB/s, {:.0} ns an event), p95 {:.1} ms",
        ms(median),
        mb / median.as_secs_f64(),
        median.as_nanos() as f64 / events as f64,
        ms(p95)
    );
    assert!(median < Duration::from_millis(500), "a 10 MB stream in under 500 ms");
    // A model streams about 100 events a second, so one event must cost a tiny share of a core.
    assert!(median.as_nanos() as f64 / (events as f64) < 20_000., "under 20 µs an event");
    let _ = took;
}

/// A model that answers at once: `calls` rounds of one tool call each, then a closing text.
struct Script {
    calls: usize,
    at: AtomicUsize,
}

impl Model for Script {
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), _: &Cancel) -> Result<Reply, ModelError> {
        let n = self.at.fetch_add(1, Ordering::SeqCst);
        // The request is built for every call, as a real one is; touch it so it is not optimised away.
        assert!(!request.messages.is_empty());
        if n < self.calls {
            let id = format!("call_{n}");
            let input = json!({"path": "note.txt"});
            sink(Delta::ToolStart { id: id.clone(), name: "read".into() });
            sink(Delta::ToolDone { id: id.clone(), input: input.clone() });
            Ok(Reply { blocks: vec![Block::ToolUse { id, name: "read".into(), input }], stop: StopReason::ToolUse, usage: TokenUsage::default(), malformed: vec![] })
        } else {
            sink(Delta::Text("done".into()));
            sink(Delta::BlockEnd);
            Ok(Reply { blocks: vec![Block::Text { text: "done".into() }], stop: StopReason::EndTurn, usage: TokenUsage::default(), malformed: vec![] })
        }
    }
}

/// A project that keeps the agent's own record out of the disk, to measure the loop apart from the writes.
struct NoRecord(Arc<dyn Project>);

impl Project for NoRecord {
    fn root(&self) -> &Path {
        self.0.root()
    }
    fn list(&self) -> io::Result<Vec<Entry>> {
        self.0.list()
    }
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0.read(path)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.0.write(path, bytes)
    }
    fn data_write(&self, _: &str, _: &[u8]) -> io::Result<()> {
        Ok(())
    }
    fn data_read(&self, path: &str) -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::NotFound, path.to_string()))
    }
    fn data_list(&self, _: &str) -> io::Result<Vec<atelier_project::DataEntry>> {
        Ok(Vec::new())
    }
    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        self.0.watch(sink)
    }
    fn search(&self, query: &Query) -> io::Result<Vec<Match>> {
        self.0.search(query)
    }
    fn spawn(&self, command: &Spawn) -> io::Result<Process> {
        self.0.spawn(command)
    }
    fn git(&self, args: &[&str]) -> io::Result<GitOutput> {
        self.0.git(args)
    }
}

/// Time from the message to the end of a turn of `calls` tool calls.
fn turn(project: Arc<dyn Project>, calls: usize) -> Duration {
    let agent = OwnAgent::new(Arc::new(Script { calls, at: AtomicUsize::new(0) }), OwnOptions::default());
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let sink: atelier_agents::session::EventSink = Arc::new(move |e| drop(tx.lock().unwrap().send(e)));
    let session = agent.open(project, OpenRequest { mode: Some(PermissionMode::Bypass), ..OpenRequest::default() }, sink).unwrap();
    // Wait for Started so the thread is up before the clock starts.
    while !matches!(rx.recv().unwrap(), Event::Started(_)) {}
    let start = Instant::now();
    session.send(Command::send("go")).unwrap();
    loop {
        if matches!(rx.recv_timeout(Duration::from_secs(60)).unwrap(), Event::TurnEnded(_)) {
            return start.elapsed();
        }
    }
}

#[test]
#[ignore = "a measuring run; needs --release"]
fn the_loop_adds_little_to_each_tool_call() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "hello\nworld\n").unwrap();
    let real: Arc<dyn Project> = Arc::new(LocalProject::open(dir.path()).unwrap());
    let bare: Arc<dyn Project> = Arc::new(NoRecord(real.clone()));
    const CALLS: usize = 100;
    let (loop_median, loop_p95) = measure(|| turn(bare.clone(), CALLS));
    let (saved_median, saved_p95) = measure(|| turn(real.clone(), CALLS));
    let per_loop = loop_median / CALLS as u32;
    let per_saved = saved_median / CALLS as u32;
    println!(
        "{CALLS} tool calls (read of a small file) with an instant model, the record not written: median {:.1} ms = {:.0} µs a call, p95 {:.1} ms",
        ms(loop_median),
        us(per_loop),
        ms(loop_p95)
    );
    println!(
        "the same with the record written after every message (a whole-file write, growing to {CALLS} calls): median {:.1} ms = {:.0} µs a call, p95 {:.1} ms",
        ms(saved_median),
        us(per_saved),
        ms(saved_p95)
    );
    assert!(per_loop < Duration::from_micros(1000), "the loop, a tool and the events under 1 ms a call");
    let _ = (Message::user(""), Cancel::default());
}
