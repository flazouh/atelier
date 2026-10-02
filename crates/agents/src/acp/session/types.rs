use std::{
    sync::{Arc, Mutex, mpsc},
};

/// The way to the writer thread, shared by the reader and the caller. Emptying it ends the writer and closes
/// the agent's stdin, whatever the reader is waiting on.
pub(in super::super) type Lines = Arc<Mutex<Option<mpsc::Sender<String>>>>;
