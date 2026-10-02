//! The microphone: the default input device, heard as 16 kHz mono audio and a live level.
//!
//! The audio thread only copies what the device gives it into a buffer and adds up its energy. Turning that into 16 kHz
//! mono, and the energy into a level for the bars, happens off the audio thread, so the callback never waits.
//!
//! cpal needs ALSA headers to build on Linux, so Linux has no capture for now: [`Recorder::start`] says so.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

use crate::recognizer::SAMPLE_RATE;

/// The longest recording kept, in seconds. The model reads one clip at a time, and a clip this long is already five minutes of
/// talking; past it the oldest audio is not kept, rather than the buffer growing for ever.
pub const MAX_SECONDS: usize = 300;

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

/// What the audio thread leaves for the rest: the audio so far, and the energy since the level was last read.
#[derive(Default)]
struct Heard {
    samples: Vec<f32>,
    energy: f64,
    count: usize,
}

impl Heard {
    /// Keeps `mixed`, and at most `cap` samples in all.
    fn push(&mut self, mixed: &[f32], cap: usize) {
        for s in mixed {
            self.energy += (*s as f64) * (*s as f64);
        }
        self.count += mixed.len();
        self.samples.extend_from_slice(mixed);
        if self.samples.len() > cap {
            let extra = self.samples.len() - cap;
            self.samples.drain(..extra);
        }
    }

    /// The RMS of what was pushed since the last call, 0 when nothing was.
    fn take_rms(&mut self) -> f32 {
        let rms = if self.count == 0 { 0. } else { (self.energy / self.count as f64).sqrt() as f32 };
        self.energy = 0.;
        self.count = 0;
        rms
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod device {
    use std::sync::{Arc, Mutex};

    use cpal::{
        FromSample, SampleFormat, SizedSample,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    };

    use super::*;
    use crate::Error;

    pub struct Recorder {
        // Dropping it ends the capture.
        stream: cpal::Stream,
        heard: Arc<Mutex<Heard>>,
        rate: u32,
    }

    fn feed<T: SizedSample>(data: &[T], channels: usize, heard: &Mutex<Heard>, cap: usize)
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

    impl Recorder {
        /// Opens the default microphone and starts listening.
        pub fn start() -> Result<Self, Error> {
            let mic = Error::Microphone;
            let device = cpal::default_host().default_input_device().ok_or_else(|| mic("no microphone found".into()))?;
            let config = device.default_input_config().map_err(|why| mic(why.to_string()))?;
            let (rate, channels, format) = (config.sample_rate(), config.channels() as usize, config.sample_format());
            let heard = Arc::new(Mutex::new(Heard::default()));
            let cap = MAX_SECONDS * rate as usize;
            let (sink, err) = (heard.clone(), |why: cpal::Error| eprintln!("microphone: {why}"));
            let config: cpal::StreamConfig = config.into();
            let stream = match format {
                SampleFormat::F32 => device.build_input_stream(config, move |d: &[f32], _: &_| feed(d, channels, &sink, cap), err, None),
                SampleFormat::I16 => device.build_input_stream(config, move |d: &[i16], _: &_| feed(d, channels, &sink, cap), err, None),
                SampleFormat::I32 => device.build_input_stream(config, move |d: &[i32], _: &_| feed(d, channels, &sink, cap), err, None),
                other => return Err(mic(format!("sample format {other} is not supported"))),
            }
            .map_err(|why| mic(why.to_string()))?;
            stream.play().map_err(|why| mic(why.to_string()))?;
            Ok(Self { stream, heard, rate })
        }

        /// The RMS of the audio since this was last asked, 0 when there was none.
        pub fn take_rms(&self) -> f32 {
            self.heard.lock().map_or(0., |mut h| h.take_rms())
        }

        /// Stops listening and gives back everything heard, at 16 kHz mono.
        pub fn finish(self) -> Vec<f32> {
            let Self { stream, heard, rate } = self;
            drop(stream);
            let samples = heard.lock().map(|mut h| std::mem::take(&mut h.samples)).unwrap_or_default();
            to_16k(&samples, rate)
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod device {
    use crate::Error;

    pub struct Recorder;

    impl Recorder {
        pub fn start() -> Result<Self, Error> {
            Err(Error::Microphone("recording is not built for this system yet".into()))
        }

        pub fn take_rms(&self) -> f32 {
            0.
        }

        pub fn finish(self) -> Vec<f32> {
            Vec::new()
        }
    }
}

pub use device::Recorder;

#[cfg(test)]
mod tests;
