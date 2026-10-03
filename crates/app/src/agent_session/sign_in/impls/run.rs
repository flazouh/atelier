use std::sync::atomic::Ordering;

use super::super::structs::Run;

impl Drop for Run {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
