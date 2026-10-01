//! The numbers M5 asks for: 300 files and 5,000 comments, open to first paint from the local cache, and
//! the cost of a frame. Run in release: `cargo test --release -p atelier-pr-view perf -- --ignored --nocapture`.
//! The frame is drawn on the test platform, so it counts layout and paint work but not the GPU.
use std::time::{Duration, Instant};

use gpui_kit::TestAppContext;

use super::view::{Harness, open, settle, setup};
use crate::fixture::{big::Big, sample};

const FILES: usize = 300;
const THREADS: usize = 1_000;
const PER_THREAD: usize = 5;

fn big() -> Harness {
    let b = Big::build(FILES, THREADS, PER_THREAD);
    Harness::over(b.repo, b.forge, b.reference, b.head)
}

fn stats(mut v: Vec<Duration>) -> String {
    v.sort();
    format!("median {:?}  p95 {:?}  max {:?}", v[v.len() / 2], v[v.len() * 95 / 100], v.last().unwrap())
}

#[gpui_kit::test]
#[ignore = "a measuring run; needs --release"]
fn perf_three_hundred_files_and_five_thousand_comments(cx: &mut TestAppContext) {
    setup(cx);
    let h = big();
    // First open: fills the git cache and the snapshot.
    {
        let started = Instant::now();
        let (view, cx) = open(&h, cx);
        settle(&view, cx, |v| v.current_view().is_some() && v.first_load_done);
        println!("first open, cold cache: {:?}", started.elapsed());
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(500));
        cx.run_until_parked();
    }
    // Second open: the cache and the snapshot are on disk.
    let started = Instant::now();
    let (view, cx) = open(&h, cx);
    println!("  window and view made: {:?}", started.elapsed());
    settle(&view, cx, |v| v.current_view().is_some());
    view.read_with(cx, |v, _| {
        for (name, at) in &v.timeline {
            println!("  {name}: {at:?}");
        }
    });
    let to_first_paint = started.elapsed();
    println!("open to first file shown, warm cache: {to_first_paint:?}");
    settle(&view, cx, |v| v.first_load_done);
    view.read_with(cx, |v, _| {
        assert_eq!(v.model.files().len(), FILES);
        assert_eq!(v.model.data.threads.len(), THREADS);
    });
    let mut git_times: Vec<Duration> = Vec::new();
    // The pieces of the opening, timed outside the test scheduler (which runs background work in turn).
    {
        let git = &h.services.git;
        let pull = h.forge.data(&h.reference).unwrap().pull.unwrap();
        let mut time = |name: &str, f: &mut dyn FnMut()| {
            let mut v = Vec::new();
            for _ in 0..7 {
                let s = Instant::now();
                f();
                v.push(s.elapsed());
            }
            println!("  {name}: {}", stats(v.clone()));
            v.sort();
            git_times.push(v[v.len() / 2]);
        };
        let prepared = git.prepare(&pull).unwrap();
        time("git prepare (cache warm)", &mut || {
            git.prepare(&pull).unwrap();
        });
        let entries = git.files(&prepared, &prepared.merge_base).unwrap();
        time("git files (300)", &mut || {
            git.files(&prepared, &prepared.merge_base).unwrap();
        });
        time("git commits", &mut || {
            git.commits(&prepared, &prepared.merge_base).unwrap();
        });
        let e = entries[0].clone();
        time("git blobs (one file)", &mut || {
            git.blobs(&prepared, &[e.old_blob.as_deref(), e.new_blob.as_deref()]).unwrap();
        });
        let blobs = git.blobs(&prepared, &[e.old_blob.as_deref(), e.new_blob.as_deref()]).unwrap();
        time("diff of one file", &mut || {
            let _ = crate::diff::FileView::build(&e, &blobs[0], &blobs[1]);
        });
    }
    {
        // What a frame asks of the model, one by one.
        let each = |name: &str, f: &mut dyn FnMut(&crate::view::PullView)| {
            let mut v = Vec::new();
            for _ in 0..30 {
                let s = Instant::now();
                view.read_with(cx, |view, _| f(view));
                v.push(s.elapsed());
            }
            println!("  per frame, {name}: {}", stats(v));
        };
        each("files()", &mut |v| {
            let _ = v.model.files();
        });
        each("progress()", &mut |v| {
            let _ = v.model.progress();
        });
        each("seen_set()", &mut |v| {
            let _ = v.model.seen_set();
        });
        each("order()", &mut |v| {
            let _ = v.model.order();
        });
        each("check_rows()", &mut |v| {
            let _ = v.model.check_rows();
        });
        each("threads_in_list_order()", &mut |v| {
            let _ = v.model.threads_in_list_order();
        });
        each("place() of the file", &mut |v| {
            let _ = crate::place::place(&v.model.data.threads, &v.current_view().unwrap());
        });
    }
    let mut convo = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        view.update(cx, |v, _| {
            let _ = v.model.conversation_page(sample::NOW, crate::layout::PAGE, crate::layout::PAGE);
        });
        convo.push(started.elapsed());
    }
    println!("conversation model ({} comments): {}", THREADS * PER_THREAD, stats(convo));
    let mut frames = Vec::new();
    for _ in 0..30 {
        view.update(cx, |_, cx| cx.notify());
        let started = Instant::now();
        cx.run_until_parked();
        frames.push(started.elapsed());
    }
    println!("frame (layout+paint, test platform): {}", stats(frames.clone()));
    frames.sort();
    let git_serial: Duration = git_times.iter().sum();
    assert!(git_serial < Duration::from_millis(200), "git work before the first file: {git_serial:?}");
}
