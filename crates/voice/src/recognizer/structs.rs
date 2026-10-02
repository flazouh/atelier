use std::path::Path;

use parakeet_rs::{ParakeetTDT, Transcriber};

use crate::Error;
use super::types::{MIN_SECONDS, SAMPLE_RATE};

pub struct Recognizer(pub(super) ParakeetTDT);

impl Recognizer {
    /// Loads the model from `dir` (see [`crate::files`]). Takes a few seconds.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        ParakeetTDT::from_pretrained(dir, None).map(Self).map_err(|why| Error::Model(why.to_string()))
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
