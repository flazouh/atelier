use super::super::{Reaction, UP_TO_DATE_NOTICE, UpdateEvent, UpdateState};

/// How much of the bar the download takes; the unpacking takes the rest.
const DOWNLOAD_SHARE: f64 = 0.9;

impl UpdateState {
    /// Whether the reader asked for this update, rather than the daily look finding it.
    fn asked(&self) -> bool {
        match self {
            Self::Checking { asked } | Self::Downloading { asked, .. } => *asked,
            Self::Idle | Self::Ready { .. } | Self::Installing => false,
        }
    }

    /// The state after `event`, and what the window does about it. A look the reader asked for says how it ended; the
    /// daily look says nothing. A ready update shows as a button in the title bar, never by itself.
    pub fn apply(self, event: UpdateEvent) -> (UpdateState, Reaction) {
        let asked = self.asked();
        match (self, event) {
            (Self::Idle, UpdateEvent::Checking { user }) => (Self::Checking { asked: user }, Reaction::Nothing),
            (state, UpdateEvent::Checking { .. }) => (state, Reaction::Nothing),
            (Self::Idle | Self::Checking { .. }, UpdateEvent::Found { version, notes, user }) => {
                (Self::Downloading { version, notes: notes.unwrap_or_default(), fraction: 0., asked: asked || user }, Reaction::Nothing)
            }
            (state, UpdateEvent::Found { .. }) => (state, Reaction::Nothing),
            (Self::Downloading { version, notes, asked, .. }, UpdateEvent::Downloading { fraction }) => {
                (Self::Downloading { version, notes, fraction: fraction.clamp(0., 1.) * DOWNLOAD_SHARE, asked }, Reaction::Nothing)
            }
            (Self::Downloading { version, notes, asked, .. }, UpdateEvent::Extracting { fraction }) => {
                let done = DOWNLOAD_SHARE + fraction.clamp(0., 1.) * (1. - DOWNLOAD_SHARE);
                (Self::Downloading { version, notes, fraction: done, asked }, Reaction::Nothing)
            }
            (Self::Downloading { version, notes, .. }, UpdateEvent::Ready) => {
                (Self::Ready { version, notes }, Reaction::Nothing)
            }
            (Self::Idle | Self::Checking { .. }, UpdateEvent::Ready) => (Self::Ready { version: String::new(), notes: String::new() }, Reaction::Nothing),
            (state @ Self::Ready { .. }, UpdateEvent::Ready) => (state, Reaction::Nothing),
            (_, UpdateEvent::Installing) => (Self::Installing, Reaction::Nothing),
            (Self::Checking { asked: true }, UpdateEvent::UpToDate) => (Self::Idle, Reaction::Say(UP_TO_DATE_NOTICE.to_string())),
            (_, UpdateEvent::UpToDate) => (Self::Idle, Reaction::Nothing),
            (state, UpdateEvent::Failed { message }) => {
                let told = asked || state == Self::Installing;
                (Self::Idle, if told { Reaction::Say(format!("Could not update: {message}")) } else { Reaction::Nothing })
            }
            (_, UpdateEvent::Idle) => (Self::Idle, Reaction::Nothing),
            (state, UpdateEvent::Downloading { .. } | UpdateEvent::Extracting { .. } | UpdateEvent::Ready) => (state, Reaction::Nothing),
        }
    }
}
