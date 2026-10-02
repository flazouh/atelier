use crate::recognizer::SAMPLE_RATE;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use cpal::{FromSample, SizedSample};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::Mutex;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::structs::Heard;

/// `samples` of `channels` interleaved channels, mixed down to one by averaging.
pub fn mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect()
}

/// `samples` at `rate` Hz brought to 16 kHz by averaging the input that falls in each output sample, which also keeps what
/// sits above 8 kHz from folding back into the speech.
pub fn to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE || rate == 0 || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = rate as f64 / SAMPLE_RATE as f64;
    let out = (samples.len() as f64 / ratio).floor() as usize;
    (0..out)
        .map(|i| {
            let (start, end) = (i as f64 * ratio, (i + 1) as f64 * ratio);
            let (first, last) = (start.floor() as usize, (end.ceil() as usize).min(samples.len()));
            let mut sum = 0.;
            let mut weight = 0.;
            for (at, sample) in samples.iter().enumerate().take(last).skip(first) {
                // How much of this input sample lies inside the output sample's span.
                let w = ((at + 1) as f64).min(end) - (at as f64).max(start);
                sum += *sample as f64 * w;
                weight += w;
            }
            if weight > 0. { (sum / weight) as f32 } else { 0. }
        })
        .collect()
}

/// A level for the bars, 0 to 1, from the RMS of a stretch of audio: -55 dB and below is silence, -10 dB and above is full,
/// with the curve between lifted a little so ordinary speech lands mid-height.
pub fn level_from_rms(rms: f32) -> f32 {
    if rms <= 0. {
        return 0.;
    }
    let db = 20. * rms.log10();
    ((db + 55.) / 45.).clamp(0., 1.).powf(0.8)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn feed<T: SizedSample>(data: &[T], channels: usize, heard: &Mutex<Heard>, cap: usize)
where
    f32: FromSample<T>,
{
    let floats: Vec<f32> = data.iter().map(|s| <f32 as FromSample<T>>::from_sample_(*s)).collect();
    let mixed = mono(&floats, channels);
    // `try_lock`: if the reader holds it for a moment, this block is dropped rather than the audio thread waiting.
    if let Ok(mut heard) = heard.try_lock() {
        heard.push(&mixed, cap);
    }
}

/// A device's name with how it connects in brackets, as the system's own sound settings say it ("Built-in", "USB", "Virtual").
pub fn label(name: &str, connection: Option<&str>) -> String {
    match connection {
        Some(how) => format!("{name} ({how})"),
        None => name.to_string(),
    }
}

/// The loudest sample, 0 to 1.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0., |loudest, s| s.abs().max(loudest))
}

/// The sample rate and mono samples (-1 to 1) of a 16-bit PCM WAV; `None` for anything else.
pub(super) fn wav(bytes: &[u8]) -> Option<(u32, Vec<f32>)> {
    if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut at, mut format) = (12, None);
    while at + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = bytes.get(at + 8..(at + 8 + size).min(bytes.len()))?;
        match &bytes[at..at + 4] {
            b"fmt " => {
                let channels = u16::from_le_bytes(body.get(2..4)?.try_into().ok()?) as usize;
                let rate = u32::from_le_bytes(body.get(4..8)?.try_into().ok()?);
                let bits = u16::from_le_bytes(body.get(14..16)?.try_into().ok()?);
                format = (bits == 16 && channels > 0).then_some((rate, channels));
            }
            b"data" => {
                let (rate, channels) = format?;
                let samples: Vec<f32> = body.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.).collect();
                return Some((rate, mono(&samples, channels)));
            }
            _ => {}
        }
        at += 8 + size + size % 2;
    }
    None
}
