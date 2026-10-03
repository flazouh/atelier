use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc,
        Mutex,
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
    kept,
    recognizer::Recognizer,
};
use super::helpers::{join, pause_end, speech_only, verdict};
use super::types::{Command, Event, LEVEL_EVERY, MAX_PRESS, PARTIAL_EVERY, PROGRESS_EVERY, Press, Reading};

pub struct Engine {
    commands: mpsc::Sender<Command>,
    loaded: Arc<AtomicBool>,
    next: Arc<AtomicU64>,
}

impl Engine {
    /// Starts the worker, which keeps waiting recordings in [`kept::dir`]. `emit` is called on the worker's thread, so it
    /// should hand the event on and return.
    pub fn spawn(emit: impl Fn(Event) + Send + 'static) -> Self {
        Self::spawn_keeping(emit, kept::dir())
    }

    /// Starts the worker, keeping waiting recordings in `kept` (none: in memory only). The first thing it does is take up
    /// the recordings an earlier run left there.
    pub fn spawn_keeping(emit: impl Fn(Event) + Send + 'static, kept: Option<PathBuf>) -> Self {
        let (commands, inbox) = mpsc::channel();
        let loaded = Arc::new(AtomicBool::new(false));
        let next = Arc::new(AtomicU64::new(1));
        let worker = Worker {
            inbox,
            outbox: commands.clone(),
            emit: Box::new(emit),
            recognizer: None,
            loaded: loaded.clone(),
            setting_up: false,
            live: None,
            waiting: VecDeque::new(),
            kept,
            next: next.clone(),
        };
        thread::Builder::new().name("atelier-dictation".into()).spawn(move || worker.run()).ok();
        Self { commands, loaded, next }
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
    /// to listen on (see [`crate::devices`]); `None` for the system's default. `tag` says whose the press is, so words kept
    /// across a quit can find their way back ([`Event::Recovered`]).
    pub fn start(&self, device: Option<String>, tag: impl Into<String>) -> Press {
        let press = self.next.fetch_add(1, Ordering::Relaxed);
        self.commands.send(Command::Start(press, device, tag.into())).ok();
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
    tag: String,
    recorder: Recorder,
    since: Instant,
    /// Where the stretch still going starts, in 16 kHz samples; what came before was cut at pauses and read.
    cut: usize,
    /// The words of the stretches before the cut.
    said: Vec<String>,
    /// A reading is on its way; the next waits for it.
    reading: bool,
    next_reading: Instant,
}

/// A recording that waits for the model, and its file on disk if it has one.
struct Waiting {
    press: Press,
    samples: Vec<f32>,
    file: Option<PathBuf>,
}

impl Waiting {
    /// Its words are out, or it was cancelled: the file goes.
    fn forget(&self) {
        if let Some(file) = &self.file {
            std::fs::remove_file(file).ok();
        }
    }
}

struct Worker {
    inbox: mpsc::Receiver<Command>,
    /// The setup thread reports back through this.
    outbox: mpsc::Sender<Command>,
    emit: Box<dyn Fn(Event) + Send>,
    /// Shared with the thread that reads a press while it records.
    recognizer: Option<Arc<Mutex<Recognizer>>>,
    loaded: Arc<AtomicBool>,
    setting_up: bool,
    live: Option<Live>,
    /// Recordings that ended before the model was ready, oldest first.
    waiting: VecDeque<Waiting>,
    /// Where waiting recordings are kept on disk; none: in memory only.
    kept: Option<PathBuf>,
    next: Arc<AtomicU64>,
}

impl Worker {
    fn run(mut self) {
        // The first microphone opened in a process took 100 ms on an M4 Pro, and 60 once the audio system was awake; later
        // ones take 45. Asking for the format opens nothing, so no light comes on and no permission is asked.
        capture::prime();
        self.recover();
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
        self.read_live();
    }

    /// Reads what the live press has said so far, on a thread of its own so the level and a stop never wait for the model.
    fn read_live(&mut self) {
        let (Some(live), Some(recognizer)) = (self.live.as_mut(), self.recognizer.clone()) else { return };
        if live.reading || Instant::now() < live.next_reading {
            return;
        }
        let samples = live.recorder.snapshot();
        let (press, from) = (live.press, live.cut);
        if samples.len() <= from {
            return;
        }
        live.reading = true;
        let tx = self.outbox.clone();
        thread::Builder::new()
            .name("atelier-dictation-read".into())
            .spawn(move || {
                let mut recognizer = recognizer.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let mut read = |s: &[f32]| recognizer.transcribe(speech_only(s)).unwrap_or_default();
                let going = &samples[from..];
                let reading = match pause_end(going) {
                    Some(end) => Reading { cut: Some((from + end, read(&going[..end]))), tail: read(&going[end..]) },
                    None => Reading { cut: None, tail: read(going) },
                };
                tx.send(Command::Read { press, from, reading }).ok();
            })
            .ok();
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Warm => self.set_up(false),
            Command::Fetch => self.set_up(true),
            Command::Start(press, device, tag) => self.start(press, device.as_deref(), tag),
            Command::Stop(press) => self.stop(press),
            Command::Cancel(press) => {
                if self.live.as_ref().is_some_and(|l| l.press == press) {
                    self.live = None;
                }
                if let Some(at) = self.waiting.iter().position(|w| w.press == press) {
                    self.waiting.remove(at).inspect(Waiting::forget);
                }
                (self.emit)(Event::Cancelled(press));
            }
            Command::Progress(event) => (self.emit)(event),
            Command::Read { press, from, reading } => {
                let Some(live) = self.live.as_mut().filter(|l| l.press == press && l.cut == from) else { return };
                live.reading = false;
                live.next_reading = Instant::now() + PARTIAL_EVERY;
                if let Some((cut, words)) = reading.cut {
                    live.cut = cut;
                    live.said.push(words);
                }
                let words = join(&[live.said.as_slice(), &[reading.tail]].concat());
                if !words.is_empty() {
                    (self.emit)(Event::Partial(press, words));
                }
            }
            Command::Loaded(result) => {
                self.setting_up = false;
                match result {
                    Ok(recognizer) => {
                        self.recognizer = Some(Arc::new(Mutex::new(*recognizer)));
                        self.loaded.store(true, Ordering::Relaxed);
                        // The model that came before is of no use once this one has loaded; it was 735 MB.
                        if let Some(old) = files::legacy_dir() {
                            std::fs::remove_dir_all(old).ok();
                        }
                        (self.emit)(Event::Ready);
                        // A file goes only once its words are out, so a quit in between keeps it.
                        while let Some(waiting) = self.waiting.pop_front() {
                            self.transcribe(waiting.press, &waiting.samples);
                            waiting.forget();
                        }
                    }
                    Err(why) => (self.emit)(Event::SetupFailed(why.to_string())),
                }
            }
        }
    }

    /// Opens the microphone at once; the model comes alongside, so the person can talk while it does.
    fn start(&mut self, press: Press, device: Option<&str>, tag: String) {
        if self.live.is_some() {
            return (self.emit)(Event::Cancelled(press));
        }
        // A replay hears a file, not the microphone, so it needs no access.
        let access = if capture::replay_path().is_some() { Access::Granted } else { access::status() };
        match access {
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
                let now = Instant::now();
                self.live = Some(Live { press, tag, recorder, since: now, cut: 0, said: Vec::new(), reading: false, next_reading: now + PARTIAL_EVERY });
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
            // The stretches cut at pauses were read already: only the last one is left.
            return self.transcribe_from(press, &samples, live.cut, live.said);
        }
        let file = self.kept.as_deref().and_then(|dir| kept::save(dir, &live.tag, &samples).inspect_err(|why| eprintln!("could not keep a recording: {why}")).ok());
        self.waiting.push_back(Waiting { press, samples, file });
        (self.emit)(Event::Waiting(press));
        // A setup that failed is tried again: the words are waiting for it.
        self.set_up(true);
    }

    /// Takes up the recordings an earlier run left waiting, oldest first, and sets the model up for them.
    fn recover(&mut self) {
        let Some(dir) = self.kept.clone() else { return };
        let mut any = false;
        for file in kept::list(&dir) {
            let Ok((tag, samples)) = kept::load(&file) else {
                std::fs::remove_file(&file).ok();
                continue;
            };
            let press = self.next.fetch_add(1, Ordering::Relaxed);
            (self.emit)(Event::Recovered(press, tag));
            self.waiting.push_back(Waiting { press, samples, file: Some(file) });
            any = true;
        }
        if any {
            self.set_up(true);
        }
    }

    fn transcribe(&mut self, press: Press, samples: &[f32]) {
        self.transcribe_from(press, samples, 0, Vec::new());
    }

    /// The words of `samples`: `said` for what comes before `cut`, read already, and the rest read now.
    fn transcribe_from(&mut self, press: Press, samples: &[f32], cut: usize, mut said: Vec<String>) {
        let Some(recognizer) = self.recognizer.clone() else { return };
        (self.emit)(Event::Transcribing(press));
        let tail = recognizer.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).transcribe(speech_only(&samples[cut.min(samples.len())..]));
        let words = tail.map(|tail| {
            said.push(tail);
            join(&said)
        });
        let event = match words.map(|words| verdict(samples, words)) {
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
