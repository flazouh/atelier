//! Leaves a recording waiting on disk as a quit would, then starts an engine and shows it take the recording up: its words
//! come back under the tag, and the file goes. `cargo run --release -p atelier-voice --example recover -- /tmp/speech.wav`
//! (16 kHz mono 16-bit, as the bench takes).
use std::{sync::mpsc, time::Duration};

use atelier_voice::{Engine, Event, kept};

fn main() {
    let bytes = std::fs::read(std::env::args().nth(1).expect("a WAV file")).expect("the WAV file");
    let samples: Vec<f32> = bytes[44..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.).collect();
    let dir = kept::dir().expect("a data folder");
    let file = kept::save(&dir, "session-from-before", &samples).expect("kept");
    println!("left {} waiting", file.display());

    let (tx, events) = mpsc::channel();
    let _engine = Engine::spawn(move |event| {
        tx.send(event).ok();
    });
    loop {
        match events.recv_timeout(Duration::from_secs(60)) {
            Ok(Event::Recovered(press, tag)) => println!("recovered press {press} for {tag:?}"),
            Ok(Event::Transcript(_, words)) => {
                println!("words: {words:?}");
                break;
            }
            Ok(Event::Failed(_, why)) => return println!("failed: {why}"),
            Ok(_) => {}
            Err(_) => return println!("nothing came"),
        }
    }
    std::thread::sleep(Duration::from_millis(50));
    println!("file still there: {}", file.exists());
}
