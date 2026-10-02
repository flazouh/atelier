use crate::{capture::peak, recognizer::{MIN_SECONDS, SAMPLE_RATE}};
use super::types::{PAUSE, SILENT_BELOW, STRETCH_AT_LEAST};

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

/// Where to cut at the first pause in `samples` (16 kHz, from the last cut), once at least [`STRETCH_AT_LEAST`] has been heard:
/// [`PAUSE`] of frames quieter than the room's own noise allows. The level is the room's: three times the quietest tenth of
/// the frames, kept between a dead-silent line and one a quiet voice stays above.
pub(super) fn pause_end(samples: &[f32]) -> Option<usize> {
    const FRAME: usize = SAMPLE_RATE as usize / 50;
    let rms: Vec<f32> = samples.as_chunks::<FRAME>().0.iter().map(|f| (f.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt()).collect();
    let mut sorted = rms.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted.get(sorted.len() / 10).copied()?;
    let quiet_below = (floor * 3.).clamp(0.002, 0.012);
    let (pause, least) = (PAUSE.as_millis() as usize / 20, STRETCH_AT_LEAST.as_millis() as usize / 20);
    let (mut spoke, mut quiet) = (false, 0);
    for (i, level) in rms.iter().enumerate() {
        if *level < quiet_below {
            quiet += 1;
            // The cut goes in the middle of the pause, so the next stretch starts in quiet rather than on its first sound.
            if spoke && quiet >= pause && i + 1 >= least {
                return Some((i + 1 - pause / 2) * FRAME);
            }
        } else {
            spoke = true;
            quiet = 0;
        }
    }
    None
}

/// The words of stretches cut at pauses, as one text. The model ends each stretch as a sentence, so a stretch that goes on in
/// lower case turns the full stop before it back into a comma.
pub(super) fn join(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            if part.starts_with(|c: char| c.is_lowercase()) && out.ends_with('.') {
                out.pop();
                out.push(',');
            }
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}
