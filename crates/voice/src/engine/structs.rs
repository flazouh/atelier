use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Instant,
};

use crate::{
    Error,
    access::{self, Access},
    capture::{self, Recorder, level_from_rms},
    files::{self, BASE, FILES},
    recognizer::Recognizer,
};
use super::helpers::verdict;
use super::types::{Command, Event, LEVEL_EVERY, MAX_PRESS, PROGRESS_EVERY, Press};

pub struct Engine {
    commands: mpsc::Sender<Command>,
    loaded: Arc<AtomicBool>,
    next: AtomicU64,
}

impl Engine {
    /// Starts the worker. `emit` is called on the worker's thread, so it should hand the event on and return.
    pub fn spawn(emit: impl Fn(Event) + Send + 'static) -> Self {
        let (commands, inbox) = mpsc::channel();
        let loaded = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            inbox,
            outbox: commands.clone(),
            emit: Box::new(emit),
            recognizer: None,
            loaded: loaded.clone(),
            setting_up: false,
            live: None,
            waiting: VecDeque::new(),
        };
        thread::Builder::new().name("atelier-dictation".into()).spawn(move || worker.run()).ok();
        Self { commands, loaded, next: AtomicU64::new(1) }
    }

    /// Whether the model is in memory, so a recording turns into words as soon as it ends.
    pub fn ready(&self) -> bool {
        self.loaded.load(Ordering::Relaxed)
    }

    /// Loads the model in the background if it is on this machine.
    pub fn warm(&self) {
        self.commands.send(Command::Warm).ok();
    }

    /// Fetches the model if it is missing, then loads it, in the background: the person showed they mean to dictate.
    pub fn fetch(&self) {
        self.commands.send(Command::Fetch).ok();
    }

    /// The microphone was pressed: it opens at once, whether or not the model is here yet. `device` is the id of the microphone
    /// to listen on (see [`crate::devices`]); `None` for the system's default.
    pub fn start(&self, device: Option<String>) -> Press {
        let press = self.next.fetch_add(1, Ordering::Relaxed);
        self.commands.send(Command::Start(press, device)).ok();
        press
    }

    /// The press ended: its recording turns into words, now or once the model is ready.
    pub fn stop(&self, press: Press) {
        self.commands.send(Command::Stop(press)).ok();
    }

    /// The press is taken back: its recording is thrown away.
    pub fn cancel(&self, press: Press) {
        self.commands.send(Command::Cancel(press)).ok();
    }
}

/// The press that is recording.
struct Live {
    press: Press,
    recorder: Recorder,
    since: Instant,
}

struct Worker {
    inbox: mpsc::Receiver<Command>,
    /// The setup thread reports back through this.
    outbox: mpsc::Sender<Command>,
    emit: Box<dyn Fn(Event) + Send>,
    recognizer: Option<Recognizer>,
    loaded: Arc<AtomicBool>,
    setting_up: bool,
    live: Option<Live>,
    /// Recordings that ended before the model was ready, oldest first.
    waiting: VecDeque<(Press, Vec<f32>)>,
}

impl Worker {
    fn run(mut self) {
        // The first microphone opened in a process took 100 ms on an M4 Pro, and 60 once the audio system was awake; later
        // ones take 45. Asking for the format opens nothing, so no light comes on and no permission is asked.
        capture::prime();
        loop {
            match self.inbox.recv_timeout(LEVEL_EVERY) {
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => self.tick(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn tick(&mut self) {
        let Some(live) = &self.live else { return };
        let press = live.press;
        if live.since.elapsed() >= MAX_PRESS {
            return self.stop(press);
        }
        (self.emit)(Event::Level(press, level_from_rms(live.recorder.take_rms())));
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Warm => self.set_up(false),
            Command::Fetch => self.set_up(true),
            Command::Start(press, device) => self.start(press, device.as_deref()),
            Command::Stop(press) => self.stop(press),
            Command::Cancel(press) => {
                if self.live.as_ref().is_some_and(|l| l.press == press) {
                    self.live = None;
                }
                self.waiting.retain(|(p, _)| *p != press);
                (self.emit)(Event::Cancelled(press));
            }
            Command::Progress(event) => (self.emit)(event),
            Command::Loaded(result) => {
                self.setting_up = false;
                match result {
                    Ok(recognizer) => {
                        self.recognizer = Some(*recognizer);
                        self.loaded.store(true, Ordering::Relaxed);
                        // The model that came before is of no use once this one has loaded; it was 735 MB.
                        if let Some(old) = files::legacy_dir() {
                            std::fs::remove_dir_all(old).ok();
                        }
                        (self.emit)(Event::Ready);
                        while let Some((press, samples)) = self.waiting.pop_front() {
                            self.transcribe(press, &samples);
                        }
                    }
                    Err(why) => (self.emit)(Event::SetupFailed(why.to_string())),
                }
            }
        }
    }

    /// Opens the microphone at once; the model comes alongside, so the person can talk while it does.
    fn start(&mut self, press: Press, device: Option<&str>) {
        if self.live.is_some() {
            return (self.emit)(Event::Cancelled(press));
        }
        match access::status() {
            Access::Granted => {}
            // The system's question goes up now and blocks until it is answered, so it is asked off this thread. This press
            // cannot record; the next one can.
            Access::Unasked => {
                thread::spawn(access::ensure);
                return (self.emit)(Event::Failed(press, "Allow microphone access, then press again.".into()));
            }
            Access::Refused => return (self.emit)(Event::Failed(press, Error::Access.to_string())),
        }
        match Recorder::start(device) {
            Ok(recorder) => {
                self.live = Some(Live { press, recorder, since: Instant::now() });
                (self.emit)(Event::Listening(press));
                self.set_up(true);
            }
            Err(why) => (self.emit)(Event::Failed(press, why.to_string())),
        }
    }

    fn stop(&mut self, press: Press) {
        let Some(live) = self.live.take_if(|l| l.press == press) else { return };
        let samples = live.recorder.finish();
        if self.recognizer.is_some() {
            return self.transcribe(press, &samples);
        }
        self.waiting.push_back((press, samples));
        (self.emit)(Event::Waiting(press));
        // A setup that failed is tried again: the words are waiting for it.
        self.set_up(true);
    }

    fn transcribe(&mut self, press: Press, samples: &[f32]) {
        let Some(recognizer) = self.recognizer.as_mut() else { return };
        (self.emit)(Event::Transcribing(press));
        let event = match recognizer.transcribe(samples).map(|words| verdict(samples, words)) {
            Ok(Ok(words)) => Event::Transcript(press, words),
            Ok(Err(why)) => Event::Failed(press, why.to_string()),
            Err(why) => Event::Failed(press, why.to_string()),
        };
        (self.emit)(event);
    }

    /// Fetches (when `fetch`) and loads the model on a thread of its own, unless it is in memory or on its way.
    fn set_up(&mut self, fetch: bool) {
        if self.recognizer.is_some() || self.setting_up {
            return;
        }
        let Some(dir) = files::dir() else {
            return (self.emit)(Event::SetupFailed("this system has no folder for app data".into()));
        };
        let installed = files::installed(&dir, &FILES);
        if !installed && !fetch {
            return;
        }
        self.setting_up = true;
        let tx = self.outbox.clone();
        thread::Builder::new()
            .name("atelier-dictation-setup".into())
            .spawn(move || {
                let progress = |event| {
                    tx.send(Command::Progress(event)).ok();
                };
                let result = (|| {
                    if !installed {
                        let total = files::total_bytes(&FILES);
                        progress(Event::Download { done: 0, total });
                        let mut last = Instant::now();
                        files::install(BASE, &dir, &FILES, &mut |done, total| {
                            if done == total || last.elapsed() >= PROGRESS_EVERY {
                                last = Instant::now();
                                progress(Event::Download { done, total });
                            }
                        })?;
                    }
                    progress(Event::Prepare);
                    Recognizer::load(&dir)
                })();
                tx.send(Command::Loaded(result.map(Box::new))).ok();
            })
            .ok();
    }
}
