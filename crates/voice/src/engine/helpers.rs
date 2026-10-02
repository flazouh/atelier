use crate::{capture::peak, recognizer::{MIN_SECONDS, SAMPLE_RATE}};
use super::types::SILENT_BELOW;

/// What to tell the person when a recording gave no words, or the words when it did. A press that ends in nothing must say why:
/// a refused microphone and a mumbled word look the same from outside.
pub(super) fn verdict(samples: &[f32], words: String) -> Result<String, &'static str> {
    if !words.trim().is_empty() {
        return Ok(words);
    }
    if (samples.len() as f32) < MIN_SECONDS * SAMPLE_RATE as f32 {
        return Err("Too short. Hold the microphone a little longer.");
    }
    if peak(samples) < SILENT_BELOW {
        return Err("No sound from the microphone. Check its access in System Settings, or pick another.");
    }
    Err("No words heard. Try again a little closer.")
}
