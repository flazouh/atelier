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

const FRAME: usize = SAMPLE_RATE as usize / 50;

/// A frame is speech only above this, whatever the room. The Mac's microphone in a quiet room reads 0.001 to 0.0025, with bumps
/// that a line drawn from the room alone (three times the quiet) took for the first word; speech reads 0.03 and more.
const SPEECH_ABOVE: f32 = 0.006;

/// The loudness of each 20 ms frame, and the level below which a frame is the room's own quiet: three times the quietest tenth of
/// the frames, kept between a dead-silent line and one a quiet voice stays above.
fn levels(samples: &[f32]) -> Option<(Vec<f32>, f32)> {
    let rms: Vec<f32> = samples.as_chunks::<FRAME>().0.iter().map(|f| (f.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt()).collect();
    let mut sorted = rms.clone();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted.get(sorted.len() / 10).copied()?;
    Some((rms, (floor * 3.).clamp(0.002, 0.012)))
}

/// Where to cut at the first pause in `samples` (16 kHz, from the last cut), once at least [`STRETCH_AT_LEAST`] has been heard:
/// [`PAUSE`] of frames quieter than the room's own noise allows.
pub(super) fn pause_end(samples: &[f32]) -> Option<usize> {
    let (rms, quiet_below) = levels(samples)?;
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
            spoke |= *level >= SPEECH_ABOVE;
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

/// Sound this close together is one stretch of speech (a gap between words is shorter), in frames.
const WORD_GAP: usize = 8;
/// A stretch of speech has at least this many loud frames: a click or a tap on the desk has fewer.
const SPEECH_AT_LEAST: usize = 4;
/// What is kept around the speech, in frames: a word starts and ends softer than the line that finds it.
const LEAD: usize = 10;
const TRAIL: usize = 15;

/// The part of `samples` that holds speech, from the first stretch of it to the last, with a little room around it; nothing when
/// it is all room. The model puts words to what is not speech ("Yeah.", "Mm-hmm.") when it is handed a breath, the click of the
/// key or a quiet that goes on, so it is never handed those.
pub(super) fn speech_only(samples: &[f32]) -> &[f32] {
    let Some((rms, quiet_below)) = levels(samples) else { return &[] };
    // Stretches of loud frames: (first, last, how many).
    let mut stretches: Vec<(usize, usize, usize)> = Vec::new();
    for (i, level) in rms.iter().enumerate() {
        if *level < quiet_below.max(SPEECH_ABOVE) {
            continue;
        }
        match stretches.last_mut() {
            Some((_, last, count)) if i - *last <= WORD_GAP => {
                *last = i;
                *count += 1;
            }
            _ => stretches.push((i, i, 1)),
        }
    }
    stretches.retain(|(_, _, count)| *count >= SPEECH_AT_LEAST);
    let (Some(&(first, ..)), Some(&(_, last, _))) = (stretches.first(), stretches.last()) else { return &[] };
    let from = first.saturating_sub(LEAD) * FRAME;
    let to = ((last + 1 + TRAIL) * FRAME).min(samples.len());
    &samples[from..to]
}
