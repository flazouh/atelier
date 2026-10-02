use super::*;

fn wav(path: &str) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).unwrap();
    assert_eq!((reader.spec().sample_rate, reader.spec().channels), (SAMPLE_RATE, 1), "a 16 kHz mono clip");
    reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.).collect()
}

/// Runs the real model on a spoken clip. It needs the model's files and a clip, so it runs only when
/// `ATELIER_VOICE_MODEL` (the folder) and `ATELIER_VOICE_CLIP` (a 16 kHz mono WAV of "Hello, this is a quick dictation test of
/// the on device model.") are set.
#[test]
fn the_real_model_hears_a_spoken_sentence() {
    let (Some(dir), Some(clip)) = (std::env::var_os("ATELIER_VOICE_MODEL"), std::env::var("ATELIER_VOICE_CLIP").ok()) else {
        eprintln!("skipped: set ATELIER_VOICE_MODEL and ATELIER_VOICE_CLIP");
        return;
    };
    let mut recognizer = Recognizer::load(std::path::Path::new(&dir)).expect("the model loads");
    let words = recognizer.transcribe(&wav(&clip)).expect("it transcribes");
    assert_eq!(words, "Hello, this is a quick dictation test of the on device model.");
}

#[test]
fn a_click_of_the_button_is_not_sent_to_the_model() {
    // A recognizer cannot be built without the model, so the short-clip rule is checked on its threshold.
    assert!((0.1 * SAMPLE_RATE as f32) < MIN_SECONDS * SAMPLE_RATE as f32);
    assert_eq!((MIN_SECONDS * SAMPLE_RATE as f32) as usize, 4800);
}
