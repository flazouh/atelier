use std::path::Path;

use parakeet_rs::{ExecutionConfig, ParakeetTDT, Transcriber};

use crate::Error;
use super::types::{MIN_SECONDS, SAMPLE_RATE, THREADS};

pub struct Recognizer(pub(super) ParakeetTDT);

impl Recognizer {
    /// Loads the model from `dir` (see [`crate::files`]). Takes a few seconds.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        // Eight threads, or fewer on a machine with fewer cores: the eval ran 8 on the M4 Pro's performance cores, and 4 (the
        // loader's default) was 1.3 times slower on the same clips.
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(THREADS));
        let config = ExecutionConfig::new().with_intra_threads(threads);
        ParakeetTDT::from_pretrained(dir, Some(config)).map(Self).map_err(|why| Error::Model(why.to_string()))
    }

    /// The words in `samples` (16 kHz, mono, -1 to 1), trimmed; empty when there is too little audio to hold any.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String, Error> {
        if (samples.len() as f32) < MIN_SECONDS * SAMPLE_RATE as f32 {
            return Ok(String::new());
        }
        let result = self.0.transcribe_samples(samples.to_vec(), SAMPLE_RATE, 1, None).map_err(|why| Error::Model(why.to_string()))?;
        Ok(result.text.trim().to_string())
    }
}
