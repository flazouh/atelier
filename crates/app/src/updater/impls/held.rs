use super::super::{Held, Relaunch, RelaunchRequest};

impl Held {
    pub(super) fn new(request: Box<dyn RelaunchRequest>, relaunch: Relaunch) -> Self {
        Held { request: Some(request), relaunch }
    }

    /// The question the reader answers.
    pub fn relaunch(&self) -> Relaunch {
        self.relaunch
    }

    /// The reader's answer: yes restarts the app so that the update installs, no keeps it as it is.
    pub fn answer(mut self, restart: bool) {
        let Some(request) = self.request.take() else { return };
        if restart { request.proceed() } else { request.decline() }
    }
}

impl Drop for Held {
    /// A question that goes away unanswered (its window closed) is a no: the updater is never left waiting.
    fn drop(&mut self) {
        if let Some(request) = self.request.take() {
            request.decline();
        }
    }
}
