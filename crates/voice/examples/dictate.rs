//! Runs one press from a terminal: fetches and loads the model if need be, listens for the seconds given (default 4), then prints
//! the words. `cargo run --release -p atelier-voice --example dictate -- 5`
use std::{sync::mpsc, time::Duration};

use atelier_voice::{Engine, Event};

fn main() {
    let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let (tx, events) = mpsc::channel();
    let engine = Engine::spawn(move |event| {
        tx.send(event).ok();
    });
    engine.start();
    let mut peak = 0f32;
    let mut last_percent = u64::MAX;
    loop {
        match events.recv_timeout(Duration::from_secs(1)) {
            Ok(Event::Download { done, total }) => {
                let percent = done * 100 / total.max(1);
                if percent != last_percent && percent % 5 == 0 {
                    last_percent = percent;
                    eprintln!("download {percent}% ({} / {} MB)", done / 1_000_000, total / 1_000_000);
                }
            }
            Ok(Event::Level(level)) => peak = peak.max(level),
            Ok(Event::Listening) => {
                eprintln!("listening for {seconds} s: speak now");
                let engine_stop = std::time::Instant::now() + Duration::from_secs(seconds);
                while std::time::Instant::now() < engine_stop {
                    if let Ok(Event::Level(level)) = events.recv_timeout(Duration::from_millis(50)) {
                        peak = peak.max(level);
                    }
                }
                eprintln!("peak level {peak:.2}");
                engine.stop();
            }
            Ok(Event::Transcript(words)) => return println!("transcript: {words:?}"),
            Ok(Event::Failed(why)) => return println!("failed: {why}"),
            Ok(other) => eprintln!("{other:?}"),
            Err(_) => {}
        }
    }
}
