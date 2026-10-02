//! Times what a press waits for, on this machine: opening the microphone until its audio flows, loading the model, and
//! turning speech into words. The speech is a 16 kHz mono 16-bit WAV, made on macOS with
//! `say -o /tmp/speech.wav --file-format=WAVE --data-format=LEI16@16000 "..."`.
//! `cargo run --release -p atelier-voice --example bench -- /tmp/speech.wav`; with `PRIME=1` it wakes the audio system first,
//! as the engine does when it starts, and runs the model once on silence.
use std::time::{Duration, Instant};

use atelier_voice::{capture::Recorder, files, recognizer::Recognizer};

fn main() {
    let path = std::env::args().nth(1).expect("a WAV file");
    let samples = wav(&std::fs::read(&path).expect("the WAV file"));
    let seconds = samples.len() as f32 / 16_000.;

    eprintln!("microphone access: {:?}", atelier_voice::access::status());
    if std::env::var_os("PRIME").is_some() {
        let at = Instant::now();
        atelier_voice::capture::prime();
        println!("primed in {} ms", at.elapsed().as_millis());
    }
    for _ in 0..3 {
        let at = Instant::now();
        match Recorder::start(None) {
            Ok(recorder) => {
                let opened = at.elapsed();
                while recorder.heard() == 0 && at.elapsed() < Duration::from_secs(2) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                println!("microphone: open in {} ms, audio flowing after {} ms", opened.as_millis(), at.elapsed().as_millis());
                recorder.finish();
            }
            Err(why) => println!("microphone: {why}"),
        }
    }

    let dir = files::dir().expect("a model folder");
    let at = Instant::now();
    let mut recognizer = Recognizer::load(&dir).expect("the model");
    println!("model: loaded in {} ms", at.elapsed().as_millis());
    if std::env::var_os("PRIME").is_some() {
        let at = Instant::now();
        recognizer.transcribe(&vec![0.; 16_000]).ok();
        println!("model: warmed in {} ms", at.elapsed().as_millis());
    }
    for round in 0..4 {
        let at = Instant::now();
        let words = recognizer.transcribe(&samples).expect("words");
        let took = at.elapsed().as_millis();
        println!("words: {seconds:.1} s of speech in {took} ms{}", if round == 0 { format!(": {words:?}") } else { String::new() });
    }
}

/// The samples of a 16-bit PCM WAV, as -1 to 1.
fn wav(bytes: &[u8]) -> Vec<f32> {
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"data" {
            let end = (at + 8 + size).min(bytes.len());
            return bytes[at + 8..end].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.).collect();
        }
        at += 8 + size + size % 2;
    }
    panic!("no data chunk")
}
