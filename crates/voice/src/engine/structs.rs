use std::{
    sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, RecvTimeoutError}},
    thread,
    time::Instant,
};

use crate::{
    Error,
    access::{self, Access},
    capture::{Recorder, level_from_rms},
    files::{self, BASE, FILES},
    recognizer::Recognizer,
};
use super::helpers::verdict;
use super::types::{Command, Event, LEVEL_EVERY, PROGRESS_EVERY, READY_BEAT};

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

    /// The microphone was pressed. `device` is the id of the microphone to listen on (see [`crate::devices`]); `None` for the
    /// system's default.
    pub fn start(&self, device: Option<String>) {
        self.commands.send(Command::Start(device)).ok();
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
                Command::Start(device) => {
                    if let Err(why) = self.press(device.as_deref()) {
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
            // The model that came before is of no use once this one has loaded; it was 735 MB.
            if let Some(old) = files::legacy_dir() {
                std::fs::remove_dir_all(old).ok();
            }
        }
        Ok(())
    }

    /// Everything from a press to the words.
    pub(super) fn press(&mut self, device: Option<&str>) -> Result<(), Error> {
        // Ask for the microphone first: the question is the person's to answer, and it should not wait behind a download.
        if access::ensure() != Access::Granted {
            return Err(Error::Access);
        }
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

        let recorder = Recorder::start(device)?;
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
        match verdict(&samples, words) {
            Ok(words) => (self.emit)(Event::Transcript(words)),
            Err(why) => (self.emit)(Event::Failed(why.to_string())),
        }
        Ok(())
    }
}
