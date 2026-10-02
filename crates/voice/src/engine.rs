//! The one worker thread dictation runs on. The app says [`Engine::start`] and [`Engine::stop`] and hears [`Event`]s; the
//! fetching, the loading, the listening and the recognizing all happen here, off the UI thread.
//!
//! A press of the microphone goes through up to four steps, and the app is told of each:
//!
//! 1. The model is not on this machine: it is fetched ([`Event::Download`]), checked, and kept.
//! 2. The model is not in memory: it loads ([`Event::Prepare`], a few seconds).
//! 3. If either of those ran, [`Event::Ready`], and a beat for the person to see it.
//! 4. The microphone opens ([`Event::Listening`]) and its level streams in ([`Event::Level`]) until stop.
//!
//! Stop turns the recording into words ([`Event::Transcribing`], then [`Event::Transcript`]). Anything that fails ends the
//! press with [`Event::Failed`], in words for the person.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    Error,
    capture::{Recorder, level_from_rms},
    files::{self, BASE, FILES},
    recognizer::Recognizer,
};

/// How often the level is read while it listens.
pub const LEVEL_EVERY: Duration = Duration::from_millis(33);
/// How often the download's progress is reported, at most.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(50);
/// How long "ready" stays up before the microphone opens, after a setup.
pub const READY_BEAT: Duration = Duration::from_millis(600);

/// What the worker tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `done` of `total` bytes of the model are on this machine.
    Download { done: u64, total: u64 },
    /// The model is on this machine and loads.
    Prepare,
    /// The setup is finished; the microphone opens in a moment.
    Ready,
    /// The microphone is open.
    Listening,
    /// How loud it is now, 0 to 1.
    Level(f32),
    /// The recording is being turned into words.
    Transcribing,
    /// The words. Empty when nothing was said.
    Transcript(String),
    /// The press ended without words, for this reason.
    Failed(String),
}

enum Command {
    Warm,
    Start,
    Stop,
}

pub struct Engine {
    commands: mpsc::Sender<Command>,
    loaded: Arc<AtomicBool>,
}

impl Engine {
    /// Starts the worker. `emit` is called on the worker's thread, so it should hand the event on and return.
    pub fn spawn(emit: impl Fn(Event) + Send + 'static) -> Self {
        let (commands, inbox) = mpsc::channel();
        let loaded = Arc::new(AtomicBool::new(false));
        let flag = loaded.clone();
        thread::Builder::new().name("atelier-dictation".into()).spawn(move || Worker { inbox, emit: Box::new(emit), recognizer: None, loaded: flag }.run()).ok();
        Self { commands, loaded }
    }

    /// Whether the model is in memory, so a press goes straight to listening.
    pub fn ready(&self) -> bool {
        self.loaded.load(Ordering::Relaxed)
    }

    /// Loads the model in the background if it is on this machine, so the first press does not wait for it.
    pub fn warm(&self) {
        self.commands.send(Command::Warm).ok();
    }

    /// The microphone was pressed.
    pub fn start(&self) {
        self.commands.send(Command::Start).ok();
    }

    /// The stop square was pressed.
    pub fn stop(&self) {
        self.commands.send(Command::Stop).ok();
    }
}

struct Worker {
    inbox: mpsc::Receiver<Command>,
    emit: Box<dyn Fn(Event) + Send>,
    recognizer: Option<Recognizer>,
    loaded: Arc<AtomicBool>,
}

impl Worker {
    fn run(mut self) {
        while let Ok(command) = self.inbox.recv() {
            match command {
                Command::Warm => {
                    if let Some(dir) = files::dir().filter(|d| files::installed(d, &FILES)) {
                        self.load(&dir).ok();
                    }
                }
                Command::Start => {
                    if let Err(why) = self.press() {
                        (self.emit)(Event::Failed(why.to_string()));
                    }
                }
                Command::Stop => {}
            }
        }
    }

    fn load(&mut self, dir: &std::path::Path) -> Result<(), Error> {
        if self.recognizer.is_none() {
            self.recognizer = Some(Recognizer::load(dir)?);
            self.loaded.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Everything from a press to the words.
    fn press(&mut self) -> Result<(), Error> {
        let dir = files::dir().ok_or_else(|| Error::Model("this system has no folder for app data".into()))?;
        let mut set_up = false;
        if !files::installed(&dir, &FILES) {
            set_up = true;
            let total = files::total_bytes(&FILES);
            (self.emit)(Event::Download { done: 0, total });
            let mut last = Instant::now();
            files::install(BASE, &dir, &FILES, &mut |done, total| {
                if done == total || last.elapsed() >= PROGRESS_EVERY {
                    last = Instant::now();
                    (self.emit)(Event::Download { done, total });
                }
            })?;
        }
        if self.recognizer.is_none() {
            set_up = true;
            (self.emit)(Event::Prepare);
            self.load(&dir)?;
        }
        if set_up {
            (self.emit)(Event::Ready);
            thread::sleep(READY_BEAT);
        }

        let recorder = Recorder::start()?;
        (self.emit)(Event::Listening);
        loop {
            match self.inbox.recv_timeout(LEVEL_EVERY) {
                Ok(Command::Stop) => break,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => (self.emit)(Event::Level(level_from_rms(recorder.take_rms()))),
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
        let samples = recorder.finish();
        (self.emit)(Event::Transcribing);
        let words = match self.recognizer.as_mut() {
            Some(recognizer) => recognizer.transcribe(&samples)?,
            None => String::new(),
        };
        (self.emit)(Event::Transcript(words));
        Ok(())
    }
}
