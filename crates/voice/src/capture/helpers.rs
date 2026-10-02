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
