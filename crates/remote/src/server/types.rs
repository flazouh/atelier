use std::{io::Write, sync::{Arc, Mutex}};

/// What a helper of another protocol says to the app, in words a reader acts on: no protocol numbers.
pub(super) const UPDATE_WORDS: &str = "the app and the atelier helper on this host are different versions: update atelier";

pub(super) type Out = Arc<Mutex<Box<dyn Write + Send>>>;
