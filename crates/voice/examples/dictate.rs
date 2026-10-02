//! Runs one press from a terminal (with `ATELIER_DICTATION_REPLAY=<wav>`, the press hears that file instead): listens at once for the seconds given (default 4) while the model is fetched and loaded if
//! need be, then prints the words. `cargo run --release -p atelier-voice --example dictate -- 5 [device id]`; it lists the microphones first
use std::{sync::mpsc, time::Duration};

use atelier_voice::{Engine, Event};

fn main() {
    let seconds: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    let (tx, events) = mpsc::channel();
    let engine = Engine::spawn(move |event| {
        tx.send(event).ok();
    });
    eprintln!("microphone access: {:?}", atelier_voice::access::status());
    for device in atelier_voice::devices() {
        eprintln!("{} {} [{}]", if device.is_default { "*" } else { " " }, device.label, device.id);
    }
    let pressed = std::time::Instant::now();
    let mut stopped = pressed;
    let press = engine.start(std::env::args().nth(2), "dictate");
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
            Ok(Event::Level(_, level)) => peak = peak.max(level),
            Ok(Event::Listening(_)) => {
                eprintln!("listening {} ms after the press, for {seconds} s: speak now", pressed.elapsed().as_millis());
                let engine_stop = std::time::Instant::now() + Duration::from_secs(seconds);
                while std::time::Instant::now() < engine_stop {
                    match events.recv_timeout(Duration::from_millis(50)) {
                        Ok(Event::Level(_, level)) => peak = peak.max(level),
                        Ok(Event::Partial(_, words)) => eprintln!("{:>5} ms  {words}", pressed.elapsed().as_millis()),
                        _ => {}
                    }
                }
                eprintln!("peak level {peak:.2}");
                stopped = std::time::Instant::now();
                engine.stop(press);
            }
            Ok(Event::Transcript(_, words)) => {
                return println!("transcript: {words:?} ({} ms after the press, {} ms after the stop)", pressed.elapsed().as_millis(), stopped.elapsed().as_millis());
            }
            Ok(Event::Failed(_, why)) => return println!("failed: {why}"),
            Ok(other) => eprintln!("{other:?}"),
            Err(_) => {}
        }
    }
}
