use std::time::Duration;

use gpui_kit::SharedString;

/// How often the pane looks for what a provider's subscription has told. Looking is a read of a local queue.
pub(super) const POLL: Duration = Duration::from_millis(300);

/// The page the pane asks for when the provider names no limit, and the most it asks for when it names a large one.
pub(super) const PAGE_FALLBACK: u32 = 30;
pub(super) const PAGE_MOST: u32 = 50;

/// How many pages of threads the pane reads in one go before it stops asking; a mailbox of tens of thousands would otherwise
/// hold the reading for as long as it takes.
pub(super) const THREAD_PAGES: usize = 20;

/// The width of the thread list, in design pixels. The reading pane takes the rest.
pub(super) const LIST_WIDTH: f32 = 300.;

pub(super) enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}

/// What went wrong with the provider that the reader can act on or wait out. What is already read stays on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// There is no connection: a banner with Retry.
    Offline,
    /// The provider asks for a wait, in milliseconds: a banner with the wait.
    Wait(u64),
    /// The reader is not signed in: an empty state with a button to Settings.
    SignedOut,
}

/// How the screen answers an error of a call. See [`react`](super::helpers::react).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reaction {
    Raise(Problem),
    /// The call is not offered, so its control goes.
    Hide,
    /// One line, in words.
    Line(String),
}

/// The small menu that is open over the reading pane's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Menu {
    Move,
    Label,
}

/// What the pane asks of the app.
pub enum MailPaneEvent {
    /// The reader asked for Settings, to connect or sign in to an account.
    OpenSettings,
}

/// What a press on a control does, as the drawing code is handed it.
pub(super) type Press = std::rc::Rc<dyn Fn(&mut gpui_kit::Window, &mut gpui_kit::App)>;

/// What a press on an address of a body asks for, with the address.
pub(super) type OpenAddress = std::rc::Rc<dyn Fn(&str, &mut gpui_kit::Window, &mut gpui_kit::App)>;
