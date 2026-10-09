use std::{rc::Rc, sync::Arc};

use atelier_capabilities::{
    Actor, CapResult, Ref, Subscription as Told,
    mail::{Draft, MailCapabilities, MailEvent, MailOperation, MailProvider, Message, Thread},
};
use atelier_ui::{ActiveTheme, scale::px, theme::radius};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ListState, ParentElement, Render, ScrollHandle, SharedString, Styled,
    Subscription, Window,
    component::input::{InputEvent, InputState, TextareaState},
    div,
};

use super::helpers::{new_list, no_account, signed_out};
use super::types::{Load, MailPaneEvent, Menu, Problem};
use crate::mail::{
    map::{BoxRow, MessageView, ThreadRow},
    source::{Choice, MailSource},
};

/// One mail account: the provider, what it can do, and the mailboxes it lists. The sidebar draws these.
pub struct Account {
    pub choice: Choice,
    pub boxes: Vec<BoxRow>,
    /// What stops the mailboxes from being read, when the reader can act on it or wait it out.
    pub problem: Option<Problem>,
    /// What went wrong with reading the mailboxes, in words, when it is neither of the above.
    pub failed: Option<SharedString>,
    pub(super) reading: bool,
    pub(super) provider: Arc<dyn MailProvider>,
    pub(super) caps: MailCapabilities,
}

impl Account {
    pub(super) fn new(choice: Choice, provider: Arc<dyn MailProvider>) -> Self {
        Self {
            choice,
            boxes: Vec::new(),
            problem: None,
            failed: None,
            reading: true,
            provider,
            caps: MailCapabilities::default(),
        }
    }
}

/// What the reply box under the thread is made of, for the drawing code.
pub(super) struct Compose {
    /// `To ana@example.com`.
    pub to: SharedString,
    /// Where the draft stands, in a few words: saved, changed since, or nothing yet.
    pub status: Option<SharedString>,
    pub input: Entity<TextareaState>,
    pub focus: FocusHandle,
    /// The words can be changed: always for a draft that is new, and for a saved one when the provider lists `update_draft`.
    pub editable: bool,
    /// A call is out, so the box and the buttons wait.
    pub busy: bool,
    pub empty: bool,
    pub save: Option<super::types::Press>,
    pub send: Option<super::types::Press>,
}

/// What reading an account's mailboxes brings.
pub(super) struct Boxes {
    pub caps: MailCapabilities,
    pub rows: Vec<BoxRow>,
}

/// What one reading of a mailbox's threads brings: the rows, newest first, and the cursor of the page after them.
pub(super) struct Fetched {
    pub rows: Vec<ThreadRow>,
    pub next: Option<String>,
}

/// What writing the reader's reply brings: the draft as the provider now holds it, and, when the reader asked to send, what the
/// send answered. A draft that was saved and not sent stays shown, at its new version.
pub(super) struct Outcome {
    pub draft: Draft,
    pub sent: Option<CapResult<Message>>,
}

/// What the reply box says of itself, for the drawing code and the tests.
pub(super) struct Facts {
    pub to: SharedString,
    pub status: Option<SharedString>,
    pub empty: bool,
    pub editable: bool,
}

/// What the pane keeps of a thread's reply between two visits: the draft the provider holds, and the words in the box.
pub(super) struct Held {
    pub thread: Ref,
    pub draft: Option<Draft>,
    pub text: SharedString,
}

pub struct MailPane {
    pub(super) source: MailSource,
    /// The person the screen acts for: every change and every draft is theirs.
    pub(super) me: Actor,
    pub(super) accounts: Vec<Account>,
    /// The account the pane is about: the one of the open mailbox, else the first.
    pub(super) shown: usize,
    pub(super) mailbox: Option<Ref>,
    /// The search in force, as the reader sent it with Enter.
    pub(super) query: String,
    pub(super) threads: Rc<Vec<ThreadRow>>,
    /// The cursor of the page after the last one read.
    pub(super) next: Option<String>,
    /// The list has a "Load more" row after the threads.
    pub(super) tail: bool,
    /// How many pages of threads are read, so a reading after a change keeps what the reader loaded.
    pub(super) pages: usize,
    pub(super) loading_more: bool,
    pub(super) threads_load: Load,
    pub(super) list: ListState,
    /// The open thread, and what is read of it.
    pub(super) open: Option<Ref>,
    pub(super) thread: Option<Thread>,
    pub(super) views: Rc<Vec<MessageView>>,
    pub(super) thread_load: Load,
    /// The messages the reader asked to see whole.
    pub(super) whole: Vec<Ref>,
    /// Where the messages of the open thread are scrolled to. A thread starts at its top, and a reply that was just sent shows.
    pub(super) messages_scroll: ScrollHandle,
    /// The next reading of the thread scrolls to its end: a reply was sent, and it is the last message.
    pub(super) to_end: bool,
    /// An address the reader pressed, waiting for their yes.
    pub(super) link: Option<SharedString>,
    pub(super) menu: Option<Menu>,
    /// The draft of the open thread, as the provider last gave it.
    pub(super) draft: Option<Draft>,
    pub(super) held: Vec<Held>,
    /// A change or a draft is out, so what it touches waits.
    pub(super) working: bool,
    pub(super) problem: Option<Problem>,
    pub(super) said: Option<SharedString>,
    /// The line in `said` is about reading, so a good reading clears it.
    pub(super) said_reading: bool,
    /// Counts the changes of mailbox and search, so a reading of threads that comes after one is dropped.
    pub(super) list_turn: u64,
    /// Counts the changes of thread, so a reading of a thread that comes after one is dropped.
    pub(super) thread_turn: u64,
    /// Counts the changes of the set of accounts, so a reading or a subscription of accounts since replaced is dropped.
    pub(super) epoch: u64,
    pub(super) search: Entity<InputState>,
    pub(super) compose: Entity<TextareaState>,
    pub(super) compose_focus: FocusHandle,
    /// What each account's subscription has told, read by [`Self::poll`].
    pub(super) listening: Vec<(usize, Told<MailEvent>)>,
    pub(super) _poll: Option<gpui_kit::Task<()>>,
    pub(super) focus: FocusHandle,
    pub(super) _subscriptions: Vec<Subscription>,
}

impl EventEmitter<MailPaneEvent> for MailPane {}

impl Focusable for MailPane {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl MailPane {
    /// A pane that acts for `me`, with no account yet.
    pub fn new(me: Actor, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let compose = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 10)
                .placeholder("Write a reply")
        });
        let enter = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.search_typed(cx);
                }
            },
        );
        // The buttons wait for words, so a change of the box asks for a new drawing.
        let typing = cx.subscribe_in(&compose, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            source: MailSource::from_providers([]),
            me,
            accounts: Vec::new(),
            shown: 0,
            mailbox: None,
            query: String::new(),
            threads: Rc::default(),
            next: None,
            tail: false,
            pages: 1,
            loading_more: false,
            threads_load: Load::Loading,
            list: new_list(),
            open: None,
            thread: None,
            views: Rc::default(),
            thread_load: Load::Loading,
            whole: Vec::new(),
            messages_scroll: ScrollHandle::new(),
            to_end: false,
            link: None,
            menu: None,
            draft: None,
            held: Vec::new(),
            working: false,
            problem: None,
            said: None,
            said_reading: false,
            list_turn: 0,
            thread_turn: 0,
            epoch: 0,
            search,
            compose,
            compose_focus: cx.focus_handle(),
            listening: Vec::new(),
            _poll: None,
            focus: cx.focus_handle(),
            _subscriptions: vec![enter, typing],
        }
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// The open mailbox and its account's place in [`Self::accounts`].
    pub fn open_mailbox_ref(&self) -> Option<(usize, &Ref)> {
        self.mailbox.as_ref().map(|mailbox| (self.shown, mailbox))
    }

    pub(super) fn account(&self) -> Option<&Account> {
        self.accounts.get(self.shown)
    }

    /// Whether the shown account lists `operation`. A control of a call it does not list is not drawn.
    pub(super) fn can(&self, operation: MailOperation) -> bool {
        self.account().is_some_and(|a| a.caps.can(operation))
    }

    /// What stops the pane: the last call about the open mailbox, or reading the mailboxes of its account.
    pub(super) fn effective_problem(&self) -> Option<Problem> {
        self.problem
            .or_else(|| self.account().and_then(|a| a.problem))
    }

    #[cfg(test)]
    pub fn rows(&self) -> &[ThreadRow] {
        &self.threads
    }

    #[cfg(test)]
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }

    #[cfg(test)]
    pub fn messages(&self) -> &[MessageView] {
        &self.views
    }

    /// The text the screen draws for message `at` of the open thread.
    #[cfg(test)]
    pub fn shown_body(&self, at: usize) -> SharedString {
        let view = &self.views[at];
        view.shown(self.whole.contains(&view.reference)).clone()
    }

    #[cfg(test)]
    pub fn list_state(&self) -> &ListState {
        &self.list
    }

    #[cfg(test)]
    pub fn messages_scroll(&self) -> &ScrollHandle {
        &self.messages_scroll
    }

    #[cfg(test)]
    pub fn open_thread_ref(&self) -> Option<&Ref> {
        self.open.as_ref()
    }

    #[cfg(test)]
    pub fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }

    #[cfg(test)]
    pub fn compose_input(&self) -> &Entity<TextareaState> {
        &self.compose
    }

    #[cfg(test)]
    pub fn search_input(&self) -> &Entity<InputState> {
        &self.search
    }

    #[cfg(test)]
    pub fn link_waiting(&self) -> Option<&str> {
        self.link.as_deref()
    }
}

impl Render for MailPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pane = cx.entity();
        let open_settings = {
            let pane = pane.clone();
            move |_: &mut Window, cx: &mut gpui_kit::App| {
                pane.update(cx, |_, cx| cx.emit(MailPaneEvent::OpenSettings))
            }
        };
        let focus = self.focus.clone();
        let root = || {
            div()
                .id("mail-pane")
                .debug_selector(|| "mail-pane".into())
                .key_context("Mail")
                .track_focus(&focus)
                .relative()
                .flex()
                .size_full()
                .min_w_0()
                .gap(px(atelier_ui::panel_layout::GAP))
        };
        let alone = |body: gpui_kit::AnyElement| {
            root().child(
                div()
                    .size_full()
                    .rounded(radius::lg())
                    .bg(theme.card)
                    .child(body),
            )
        };
        if self.accounts.is_empty() {
            return alone(no_account(open_settings, &theme)).into_any_element();
        }
        if self.effective_problem() == Some(Problem::SignedOut) {
            let name = self
                .account()
                .map_or_else(|| "this account".to_string(), |a| a.choice.name());
            return alone(signed_out(&name, open_settings, &theme)).into_any_element();
        }
        root()
            .child(self.list_card(cx))
            .child(self.reading_card(cx))
            .into_any_element()
    }
}
