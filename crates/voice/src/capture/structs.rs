use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use cpal::{
    SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use super::helpers::{feed, label, one_row_per_device, to_16k, wav};
use super::types::{Device, MAX_SECONDS};
use crate::Error;

/// What the audio thread leaves for the rest: the audio so far, and the energy since the level was last read.
#[derive(Default)]
pub(super) struct Heard {
    pub(super) samples: Vec<f32>,
    energy: f64,
    count: usize,
}

impl Heard {
    /// Keeps `mixed`, and at most `cap` samples in all.
    pub(super) fn push(&mut self, mixed: &[f32], cap: usize) {
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
    pub(super) fn take_rms(&mut self) -> f32 {
        let rms = if self.count == 0 { 0. } else { (self.energy / self.count as f64).sqrt() as f32 };
        self.energy = 0.;
        self.count = 0;
        rms
    }
}

pub struct Recorder {
    // Dropping it ends the capture.
    _source: Source,
    heard: Arc<Mutex<Heard>>,
    rate: u32,
}

/// Where the audio comes from: the microphone, or a file played as if spoken (see [`replay_path`]).
enum Source {
    Microphone { _stream: cpal::Stream },
    Replay(Arc<AtomicBool>),
}

impl Drop for Source {
    fn drop(&mut self) {
        if let Source::Replay(stop) = self {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// `ATELIER_DICTATION_REPLAY`: a 16-bit WAV that every press "hears" in real time instead of the microphone, and no
/// microphone access is asked for. For trying dictation where no one can speak: a test machine, a session over ssh, a box
/// with no recording built.
pub fn replay_path() -> Option<std::path::PathBuf> {
    std::env::var_os("ATELIER_DICTATION_REPLAY").map(Into::into)
}

impl Recorder {
    /// Opens the microphone `id` (from [`devices`]) and starts listening; the system's default when `id` is `None`, or is no
    /// longer plugged in.
    pub fn start(id: Option<&str>) -> Result<Self, Error> {
        if let Some(path) = replay_path() {
            return Self::replay(&path);
        }
        Self::microphone(id)
    }

    /// Plays the WAV at `path` into the recording as fast as it would be spoken.
    fn replay(path: &std::path::Path) -> Result<Self, Error> {
        let (rate, samples) = wav(&std::fs::read(path).map_err(|why| Error::Microphone(format!("{}: {why}", path.display())))?)
            .ok_or_else(|| Error::Microphone(format!("{} is not a 16-bit WAV", path.display())))?;
        let heard = Arc::new(Mutex::new(Heard::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (sink, stopped) = (heard.clone(), stop.clone());
        std::thread::spawn(move || {
            let step = rate as usize / 50;
            let begun = std::time::Instant::now();
            for (i, chunk) in samples.chunks(step).enumerate() {
                if stopped.load(Ordering::Relaxed) {
                    return;
                }
                let due = begun + std::time::Duration::from_millis(20 * i as u64);
                std::thread::sleep(due.saturating_duration_since(std::time::Instant::now()));
                if let Ok(mut h) = sink.lock() {
                    h.push(chunk, usize::MAX);
                }
            }
        });
        Ok(Self { _source: Source::Replay(stop), heard, rate })
    }

    fn microphone(id: Option<&str>) -> Result<Self, Error> {
        let mic = Error::Microphone;
        let host = cpal::default_host();
        let chosen = id.and_then(|id| id.parse::<cpal::DeviceId>().ok()).and_then(|id| host.device_by_id(&id));
        let device = chosen.or_else(|| host.default_input_device()).ok_or_else(|| mic("no microphone found".into()))?;
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
        Ok(Self { _source: Source::Microphone { _stream: stream }, heard, rate })
    }

    /// The RMS of the audio since this was last asked, 0 when there was none.
    pub fn take_rms(&self) -> f32 {
        self.heard.lock().map_or(0., |mut h| h.take_rms())
    }

    /// How many samples have come from the microphone so far, at its own rate: none until the audio really flows.
    pub fn heard(&self) -> usize {
        self.heard.lock().map_or(0, |h| h.samples.len())
    }

    /// Everything heard so far, at 16 kHz mono, while it goes on listening.
    pub fn snapshot(&self) -> Vec<f32> {
        let samples = self.heard.lock().map(|h| h.samples.clone()).unwrap_or_default();
        to_16k(&samples, self.rate)
    }

    /// Stops listening and gives back everything heard, at 16 kHz mono.
    pub fn finish(self) -> Vec<f32> {
        let Self { _source, heard, rate } = self;
        drop(_source);
        let samples = heard.lock().map(|mut h| std::mem::take(&mut h.samples)).unwrap_or_default();
        to_16k(&samples, rate)
    }
}

/// Wakes the audio system and asks for the default microphone's format, without opening it, so the first press opens it
/// faster.
pub fn prime() {
    if let Some(device) = cpal::default_host().default_input_device() {
        device.default_input_config().ok();
    }
}

/// The microphones the system offers now, the default first.
pub fn devices() -> Vec<Device> {
    use cpal::{DeviceDescription, InterfaceType};
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.id().ok()).map(|id| id.to_string());
    let connection = |d: &DeviceDescription| match d.interface_type() {
        InterfaceType::BuiltIn => Some("Built-in"),
        InterfaceType::Usb => Some("USB"),
        InterfaceType::Bluetooth => Some("Bluetooth"),
        InterfaceType::Virtual => Some("Virtual"),
        InterfaceType::Aggregate => Some("Aggregate"),
        _ => None,
    };
    let mut found: Vec<Device> = host
        .input_devices()
        .map(|all| {
            all.filter_map(|d| {
                let id = d.id().ok()?.to_string();
                let description = d.description().ok()?;
                let is_default = default.as_deref() == Some(id.as_str());
                Some(Device { id, label: label(description.name(), connection(&description)), is_default })
            })
            .collect()
        })
        .unwrap_or_default();
    found = one_row_per_device(found);
    found.sort_by_key(|d| !d.is_default);
    found
}
