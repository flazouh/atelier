#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::{Arc, Mutex};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use cpal::{
    SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::helpers::{feed, label, to_16k};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use super::types::MAX_SECONDS;
use super::types::Device;
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub struct Recorder {
    // Dropping it ends the capture.
    stream: cpal::Stream,
    heard: Arc<Mutex<Heard>>,
    rate: u32,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Recorder {
    /// Opens the microphone `id` (from [`devices`]) and starts listening; the system's default when `id` is `None`, or is no
    /// longer plugged in.
    pub fn start(id: Option<&str>) -> Result<Self, Error> {
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

/// The microphones the system offers now, the default first.
#[cfg(any(target_os = "macos", target_os = "windows"))]
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
    found.sort_by_key(|d| !d.is_default);
    found
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn devices() -> Vec<Device> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub struct Recorder;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl Recorder {
    pub fn start(_: Option<&str>) -> Result<Self, Error> {
        Err(Error::Microphone("recording is not built for this system yet".into()))
    }

    pub fn take_rms(&self) -> f32 {
        0.
    }

    pub fn finish(self) -> Vec<f32> {
        Vec::new()
    }
}
