use std::{collections::BTreeSet, rc::Rc, sync::Arc};

use atelier_capabilities::{
    CapError, CapResult, Ref, Subscription as Told,
    messaging::{
        Event, Feature, Filter, MessagingCapabilities, MessagingProvider, NewMessage, Operation,
    },
};
use atelier_ui::{
    ActiveTheme, Button, ButtonSize, ButtonVariant, PromptInput, PromptInputEvent, scale::px,
    theme::radius, typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FollowMode,
    InteractiveElement, IntoElement, KeyDownEvent, ListAlignment, ListState, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div, list, prelude::FluentBuilder,
};

use super::super::{
    map::{Group, Line, Row},
    rows::{OpenThread, draw_line},
    source::{Choice, MessagesSource},
};
use super::helpers::{
    Channels, Fetched, apply, banner, load_older, no_account, react, read_channels, read_history,
    read_older, read_thread, say, set_head, signed_out,
};
use super::types::{Load, MessagesEvent, POLL, Problem, Reaction, View};

/// One messaging account: the provider, what it can do, and the channels it lists. The sidebar draws these.
pub struct Account {
    pub choice: Choice,
    pub rows: Vec<Row>,
    /// What stops the channels from being read, when the reader can act on it or wait it out.
    pub problem: Option<Problem>,
    /// What went wrong with reading the channels, in words, when it is neither of the above.
    pub failed: Option<SharedString>,
    pub(super) reading: bool,
    pub(super) provider: Arc<dyn MessagingProvider>,
    pub(super) caps: MessagingCapabilities,
}

impl Account {
    fn new(choice: Choice, provider: Arc<dyn MessagingProvider>) -> Self {
        Self {
            choice,
            rows: Vec::new(),
            problem: None,
            failed: None,
            reading: true,
            provider,
            caps: MessagingCapabilities::default(),
        }
    }

    /// The rows of one group, in the provider's order.
    pub fn group(&self, group: Group) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(move |r| r.group == group)
    }
}

pub struct MessagesPane {
    source: MessagesSource,
    accounts: Vec<Account>,
    /// The account the pane is about: the one of the open channel, else the first.
    shown: usize,
    channel: Option<Ref>,
    view: View,
    history: Rc<Vec<Line>>,
    /// The list has a "Load older" row before the lines.
    head: bool,
    /// The cursor of the page before the oldest one read.
    older: Option<String>,
    /// How many pages of history are read, so a reading after a change keeps what the reader loaded.
    pages: usize,
    loading_older: bool,
    history_load: Load,
    thread: Rc<Vec<Line>>,
    thread_load: Load,
    problem: Option<Problem>,
    said: Option<SharedString>,
    /// The line in `said` is about reading, so a good reading clears it.
    said_reading: bool,
    /// Counts the changes of channel and thread, so a reading that comes after one is dropped.
    generation: u64,
    /// Counts the changes of the set of accounts, so a reading of channels or a subscription of accounts since replaced is dropped.
    epoch: u64,
    sending: bool,
    list: ListState,
    thread_list: ListState,
    composer: Entity<PromptInput>,
    reply_composer: Entity<PromptInput>,
    /// What each account's subscription has told, read by [`Self::poll`].
    listening: Vec<(usize, Told<Event>)>,
    _poll: Option<gpui_kit::Task<()>>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<MessagesEvent> for MessagesPane {}

impl Focusable for MessagesPane {
    /// The part the keys go to: the composer when there is one, else the pane.
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        match self.composer_shown() {
            true => self.composer_now().focus_handle(cx),
            false => self.focus.clone(),
        }
    }
}

/// A list that lays out only the rows on screen, from the bottom, and follows the end while the reader is at it.
fn new_list() -> ListState {
    let list = ListState::new(0, ListAlignment::Bottom, px(160.));
    list.set_follow_mode(FollowMode::Tail);
    list
}

impl MessagesPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| PromptInput::new("Write a message", "", window, cx));
        let reply_composer = cx.new(|cx| PromptInput::new("Reply in the thread", "", window, cx));
        let subscriptions = [&composer, &reply_composer]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(
                    input,
                    window,
                    |this: &mut Self, _, event: &PromptInputEvent, window, cx| {
                        if let PromptInputEvent::Submit(message) = event {
                            this.submit(message.text.clone(), window, cx);
                        }
                    },
                )
            })
            .collect();
        Self {
            source: MessagesSource::from_providers([]),
            accounts: Vec::new(),
            shown: 0,
            channel: None,
            view: View::Channel,
            history: Rc::default(),
            head: false,
            older: None,
            pages: 1,
            loading_older: false,
            history_load: Load::Loading,
            thread: Rc::default(),
            thread_load: Load::Loading,
            problem: None,
            said: None,
            said_reading: false,
            generation: 0,
            epoch: 0,
            sending: false,
            list: new_list(),
            thread_list: new_list(),
            composer,
            reply_composer,
            listening: Vec::new(),
            _poll: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Gives the pane the app's messaging providers. The same ones as before change nothing; others replace the accounts, and
    /// the channels of each are read, off the UI thread.
    pub fn set_providers(
        &mut self,
        providers: Vec<Arc<dyn MessagingProvider>>,
        cx: &mut Context<Self>,
    ) {
        if self.source.same_as(&providers) {
            return;
        }
        self.generation += 1;
        self.epoch += 1;
        self.source = MessagesSource::from_providers(providers);
        self.accounts = self
            .source
            .accounts()
            .into_iter()
            .map(|(choice, provider)| Account::new(choice, provider))
            .collect();
        self.listening.clear();
        (self.shown, self.channel, self.view) = (0, None, View::Channel);
        self.clear_open(cx);
        for at in 0..self.accounts.len() {
            self.read_channels(at, cx);
            self.listen(at, cx);
        }
        self.start_polling(cx);
        cx.notify();
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// The open channel and its account's place in [`Self::accounts`].
    pub fn open_channel_ref(&self) -> Option<(usize, &Ref)> {
        self.channel.as_ref().map(|channel| (self.shown, channel))
    }

    #[cfg(test)]
    pub fn lines(&self) -> &[Line] {
        match self.view {
            View::Channel => &self.history,
            View::Thread(_) => &self.thread,
        }
    }

    #[cfg(test)]
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }

    #[cfg(test)]
    pub fn composer(&self) -> &Entity<PromptInput> {
        self.composer_now()
    }

    #[cfg(test)]
    pub fn list_state(&self) -> &ListState {
        match self.view {
            View::Channel => &self.list,
            View::Thread(_) => &self.thread_list,
        }
    }

    /// Forgets what the open channel showed.
    fn clear_open(&mut self, cx: &mut Context<Self>) {
        (self.history, self.thread) = (Rc::default(), Rc::default());
        (self.list, self.thread_list) = (new_list(), new_list());
        self.head = false;
        (self.older, self.pages, self.loading_older) = (None, 1, false);
        (self.history_load, self.thread_load) = (Load::Loading, Load::Loading);
        (self.problem, self.said, self.said_reading) = (None, None, false);
        cx.notify();
    }

    fn account(&self) -> Option<&Account> {
        self.accounts.get(self.shown)
    }

    fn can(&self, operation: Operation) -> bool {
        self.account().is_some_and(|a| a.caps.can(operation))
    }

    /// Whether the provider has replies under messages, and a call to read them.
    fn threads(&self) -> bool {
        self.can(Operation::Thread) && self.account().is_some_and(|a| a.caps.has(Feature::Threads))
    }

    /// What stops the pane: the last call about the open channel, or reading the channels of its account.
    pub(super) fn effective_problem(&self) -> Option<Problem> {
        self.problem
            .or_else(|| self.account().and_then(|a| a.problem))
    }

    fn ready(&self) -> bool {
        match self.view {
            View::Channel => matches!(self.history_load, Load::Ready),
            View::Thread(_) => matches!(self.thread_load, Load::Ready),
        }
    }

    /// The composer is there only when the provider lists `send`, and the open channel or thread is read.
    pub fn composer_shown(&self) -> bool {
        self.channel.is_some()
            && self.can(Operation::Send)
            && self.effective_problem() != Some(Problem::SignedOut)
            && self.ready()
    }

    fn composer_now(&self) -> &Entity<PromptInput> {
        match self.view {
            View::Channel => &self.composer,
            View::Thread(_) => &self.reply_composer,
        }
    }

    /// Reads the channels of account `at` again, with what it can do, off the UI thread.
    fn read_channels(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(provider) = self.accounts.get(at).map(|a| a.provider.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let reading = cx.background_spawn(async move { read_channels(provider.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.channels_read(at, result, epoch, cx))
                .ok();
        })
        .detach();
    }

    fn channels_read(
        &mut self,
        at: usize,
        result: CapResult<Channels>,
        epoch: u64,
        cx: &mut Context<Self>,
    ) {
        if epoch != self.epoch {
            return;
        }
        let Some(account) = self.accounts.get_mut(at) else {
            return;
        };
        account.reading = false;
        match result {
            Ok(Channels { caps, rows }) => {
                (account.caps, account.rows) = (caps, rows);
                (account.problem, account.failed) = (None, None);
            }
            Err(error) => match react(&error) {
                Reaction::Raise(problem) => account.problem = Some(problem),
                Reaction::Hide | Reaction::Line(_) => {
                    account.failed = Some(error.to_string().into())
                }
            },
        }
        // The first channel of the first account that has one opens, so the pane has something to show.
        if self.channel.is_none()
            && let Some((first, channel)) = self
                .accounts
                .iter()
                .enumerate()
                .find_map(|(i, a)| a.rows.first().map(|r| (i, r.reference.clone())))
        {
            self.open_channel(first, &channel, cx);
        }
        cx.notify();
    }

    /// Shows the history of `channel` of account `at`, from the newest message. Pressing the open channel again closes its thread.
    pub fn open_channel(&mut self, at: usize, channel: &Ref, cx: &mut Context<Self>) {
        if at >= self.accounts.len() {
            return;
        }
        if self.shown == at && self.channel.as_ref() == Some(channel) {
            self.view = View::Channel;
            cx.notify();
            return;
        }
        self.generation += 1;
        (self.shown, self.channel, self.view) = (at, Some(channel.clone()), View::Channel);
        self.clear_open(cx);
        self.read_history(cx);
        self.mark_read(cx);
    }

    /// The reader has the channel in front of them, so the provider is told it is read, when it keeps that and the channel has
    /// unread. The channels are read again after, for what the provider now says.
    fn mark_read(&mut self, cx: &mut Context<Self>) {
        let (Some(account), Some(channel)) = (self.account(), self.channel.clone()) else {
            return;
        };
        let unread = account
            .rows
            .iter()
            .any(|r| r.reference == channel && r.unread);
        if !unread || !account.caps.can(Operation::MarkRead) {
            return;
        }
        let (provider, at, epoch) = (account.provider.clone(), self.shown, self.epoch);
        let marking = cx.background_spawn(async move { provider.mark_read(&channel, None) });
        cx.spawn(async move |this, cx| {
            // A refusal changes nothing the reader can act on: the channel stays unread.
            if marking.await.is_ok() {
                this.update(cx, |this, cx| {
                    if epoch == this.epoch {
                        this.read_channels(at, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// Reads the open channel's first pages again (as many as are read), off the UI thread.
    fn read_history(&mut self, cx: &mut Context<Self>) {
        let (Some(account), Some(channel)) = (self.account(), self.channel.clone()) else {
            return;
        };
        let (provider, pages, generation) = (account.provider.clone(), self.pages, self.generation);
        let reading = cx.background_spawn(async move {
            read_history(
                provider.as_ref(),
                &channel,
                pages,
                crate::agent_session::now(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.history_read(result, generation, cx))
                .ok();
        })
        .detach();
    }

    fn history_read(
        &mut self,
        result: CapResult<Fetched>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(Fetched { lines, next }) => {
                apply(&mut self.history, &self.list, usize::from(self.head), lines);
                self.older = next;
                set_head(&self.list, &mut self.head, self.older.is_some());
                self.history_load = Load::Ready;
                self.good_reading();
            }
            Err(error) => self.fail(error, Operation::History, "Could not read the messages", cx),
        }
        cx.notify();
    }

    fn good_reading(&mut self) {
        self.problem = None;
        if std::mem::take(&mut self.said_reading) {
            self.said = None;
        }
    }

    /// Reads the page of history before the oldest one shown, when there is one, and puts it above.
    pub fn load_older(&mut self, cx: &mut Context<Self>) {
        let (Some(provider), Some(channel), Some(cursor)) = (
            self.account().map(|a| a.provider.clone()),
            self.channel.clone(),
            self.older.clone(),
        ) else {
            return;
        };
        if self.loading_older {
            return;
        }
        self.loading_older = true;
        let generation = self.generation;
        let reading = cx.background_spawn(async move {
            read_older(
                provider.as_ref(),
                &channel,
                &cursor,
                crate::agent_session::now(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.older_read(result, generation, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn older_read(&mut self, result: CapResult<Fetched>, generation: u64, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.loading_older = false;
        match result {
            Ok(Fetched { lines, next }) => {
                // A message may sit in two pages when the channel moved between the readings.
                let held: BTreeSet<&Ref> = self.history.iter().map(|l| &l.reference).collect();
                let mut all: Vec<Line> = lines
                    .into_iter()
                    .filter(|l| !held.contains(&l.reference))
                    .collect();
                all.extend(self.history.iter().cloned());
                apply(&mut self.history, &self.list, usize::from(self.head), all);
                self.older = next;
                set_head(&self.list, &mut self.head, self.older.is_some());
                self.pages += 1;
            }
            Err(error) => self.fail(
                error,
                Operation::History,
                "Could not read older messages",
                cx,
            ),
        }
        cx.notify();
    }

    /// Shows the thread under `root` in the pane, and reads it.
    pub fn open_thread(&mut self, root: &Ref, cx: &mut Context<Self>) {
        if !self.threads() {
            return;
        }
        self.generation += 1;
        self.view = View::Thread(root.clone());
        (self.thread, self.thread_list) = (Rc::default(), new_list());
        self.thread_load = Load::Loading;
        (self.problem, self.said, self.said_reading) = (None, None, false);
        self.read_thread(cx);
        cx.notify();
    }

    fn read_thread(&mut self, cx: &mut Context<Self>) {
        let (Some(account), View::Thread(root)) = (self.account(), self.view.clone()) else {
            return;
        };
        let (provider, generation) = (account.provider.clone(), self.generation);
        let reading = cx.background_spawn(async move {
            read_thread(provider.as_ref(), &root, crate::agent_session::now())
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.thread_read(result, generation, cx))
                .ok();
        })
        .detach();
    }

    fn thread_read(&mut self, result: CapResult<Fetched>, generation: u64, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(Fetched { lines, .. }) => {
                apply(&mut self.thread, &self.thread_list, 0, lines);
                self.thread_load = Load::Ready;
                self.good_reading();
            }
            Err(error) => self.fail(error, Operation::Thread, "Could not read the thread", cx),
        }
        cx.notify();
    }

    /// Back from a thread to the channel.
    pub fn back(&mut self, cx: &mut Context<Self>) {
        if matches!(self.view, View::Thread(_)) {
            self.generation += 1;
            self.view = View::Channel;
            (self.problem, self.said, self.said_reading) = (None, None, false);
            // What the thread changed (a reply count) shows in the channel.
            self.read_history(cx);
            cx.notify();
        }
    }

    /// Reads what is shown again: the channels of the account and the open channel or thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.read_channels(self.shown, cx);
        self.refresh_open(cx);
    }

    fn refresh_open(&mut self, cx: &mut Context<Self>) {
        match self.view {
            View::Channel => self.read_history(cx),
            View::Thread(_) => self.read_thread(cx),
        }
    }

    /// Answers an error of a call as [`react`] says. `doing` starts the line that says it.
    fn fail(&mut self, error: CapError, operation: Operation, doing: &str, cx: &mut Context<Self>) {
        let words = error.to_string();
        let loading = |load: &Load| matches!(load, Load::Loading);
        // A failed send always says why, whichever way the error is answered, for the text it kept is the reader's.
        let mut says = operation == Operation::Send;
        match react(&error) {
            Reaction::Raise(problem) => {
                self.problem = Some(problem);
                // The banner sits over the pane, so a first reading that failed shows it over the empty list.
                match operation {
                    Operation::Thread if loading(&self.thread_load) => {
                        self.thread_load = Load::Ready
                    }
                    Operation::History if loading(&self.history_load) => {
                        self.history_load = Load::Ready
                    }
                    _ => {}
                }
            }
            // The control of a call the provider does not offer goes. A reading it cannot give is a plain failure.
            Reaction::Hide if matches!(operation, Operation::Send | Operation::Thread) => {
                if let Some(account) = self.accounts.get_mut(self.shown) {
                    account.caps.operations.retain(|o| *o != operation);
                }
                if operation == Operation::Thread {
                    self.view = View::Channel;
                }
            }
            Reaction::Hide | Reaction::Line(_) => {
                // A first reading that failed takes the pane, in words; any other failure is a line under the header.
                let first = match operation {
                    Operation::History if loading(&self.history_load) => {
                        Some(&mut self.history_load)
                    }
                    Operation::Thread if loading(&self.thread_load) => Some(&mut self.thread_load),
                    _ => None,
                };
                match first {
                    Some(load) => *load = Load::Failed(words.clone().into()),
                    None => says = true,
                }
            }
        }
        if says {
            self.said = Some(format!("{doing}: {words}").into());
            self.said_reading = operation != Operation::Send;
        }
        cx.notify();
    }

    /// Sends what the reader typed, to the open channel or as a reply in the open thread. The text is the reader's, so the
    /// message goes as the person; the pane never sends on its own.
    fn submit(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(account), Some(channel)) = (self.account(), self.channel.clone()) else {
            return;
        };
        if self.sending || !account.caps.can(Operation::Send) {
            self.composer_now()
                .clone()
                .update(cx, |c, cx| c.set_text(text, window, cx));
            return;
        }
        let new = match &self.view {
            View::Thread(root) => NewMessage::reply(root, &channel, &text),
            View::Channel => NewMessage::to(&channel, &text),
        };
        let provider = account.provider.clone();
        self.sending = true;
        self.said = None;
        self.set_composers_off(true, cx);
        let sending = cx.background_spawn(async move {
            let by = provider.whoami()?;
            provider.send(&new, &by)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = sending.await;
            this.update_in(cx, |this, window, cx| this.sent(result, text, window, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn set_composers_off(&mut self, off: bool, cx: &mut Context<Self>) {
        for input in [self.composer.clone(), self.reply_composer.clone()] {
            input.update(cx, |input, cx| input.set_disabled(off, cx));
        }
    }

    fn sent(
        &mut self,
        result: CapResult<atelier_capabilities::messaging::Message>,
        text: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sending = false;
        self.set_composers_off(false, cx);
        match result {
            Ok(_) => self.refresh_open(cx),
            Err(error) => {
                // The text goes back in the box, so the reader can send it again or change it.
                self.composer_now()
                    .clone()
                    .update(cx, |c, cx| c.set_text(text, window, cx));
                self.fail(error, Operation::Send, "Could not send", cx);
            }
        }
        cx.notify();
    }

    /// Asks account `at` for what happens from now on, off the UI thread.
    fn listen(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(provider) = self.accounts.get(at).map(|a| a.provider.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let asking = cx.background_spawn(async move { provider.subscribe(&Filter::default()) });
        cx.spawn(async move |this, cx| {
            // A provider that cannot follow its service (it does not list `subscribe`, or it is offline) leaves the pane to
            // the reader's own reloads.
            if let Ok(told) = asking.await {
                this.update(cx, |this, _| {
                    if epoch == this.epoch {
                        this.listening.push((at, told));
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        if self._poll.is_some() {
            return;
        }
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    /// Looks at what the subscriptions have told. A burst is one reading: of the open channel when it is the one that
    /// changed, and of the channels of an account for what the others say (their unread).
    fn poll(&mut self, cx: &mut Context<Self>) {
        let (mut open, mut elsewhere) = (false, BTreeSet::new());
        self.listening.retain_mut(|(at, told)| {
            loop {
                match told.try_recv() {
                    Ok(event) => {
                        if *at == self.shown && self.channel.as_ref() == Some(&event.channel) {
                            open = true;
                        } else {
                            elsewhere.insert(*at);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
                }
            }
        });
        if open {
            self.refresh_open(cx);
            // The reader has the channel in front of them, so what came in is read.
            elsewhere.insert(self.shown);
            self.mark_open_read(cx);
        }
        for at in elsewhere {
            self.read_channels(at, cx);
        }
    }

    /// Tells the provider the open channel is read, when it keeps that.
    fn mark_open_read(&mut self, cx: &mut Context<Self>) {
        if let (Some(account), Some(channel)) = (self.account(), self.channel.clone())
            && account.caps.can(Operation::MarkRead)
        {
            let provider = account.provider.clone();
            cx.background_spawn(async move { provider.mark_read(&channel, None).ok() })
                .detach();
        }
    }

    fn title(&self) -> SharedString {
        let Some(row) = self
            .channel
            .as_ref()
            .and_then(|c| self.account()?.rows.iter().find(|r| r.reference == *c))
        else {
            return "Messages".into();
        };
        match (&self.view, row.kind) {
            (View::Thread(_), _) => format!("Thread in {}", row.name).into(),
            (_, atelier_capabilities::messaging::ChannelKind::Public) => {
                format!("#{}", row.name).into()
            }
            _ => row.name.clone(),
        }
    }
}

impl Render for MessagesPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pane = cx.entity();
        let open_accounts = {
            let pane = pane.clone();
            move |_: &mut Window, cx: &mut gpui_kit::App| {
                pane.update(cx, |_, cx| cx.emit(MessagesEvent::OpenAccounts))
            }
        };
        let in_thread = matches!(self.view, View::Thread(_));
        let header = div()
            .id("messages-header")
            .debug_selector(|| "messages-header".into())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(12.))
            .child(
                div()
                    .debug_selector(|| "messages-title".into())
                    .text_size(TextSize::Sm.font_size())
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(self.title()),
            )
            .children(self.account().filter(|_| self.channel.is_some()).map(|a| {
                div()
                    .text_size(TextSize::Xs.font_size())
                    .text_color(theme.muted_foreground)
                    .child(a.choice.words())
            }))
            .child(div().flex_1())
            .when(in_thread, |d| {
                let pane = pane.clone();
                d.child(
                    Button::new("messages-back")
                        .debug_name("messages-back")
                        .label("Back")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .cap("Esc")
                        .on_click(move |_, _, cx| pane.update(cx, |p, cx| p.back(cx))),
                )
            });
        let problem = self.effective_problem();
        let name = self
            .account()
            .map_or_else(|| "this account".to_string(), |a| a.choice.name());
        let body = if self.accounts.is_empty() {
            no_account(open_accounts.clone(), &theme)
        } else if problem == Some(Problem::SignedOut) {
            signed_out(&name, open_accounts.clone(), &theme)
        } else if let Some(failed) = self
            .account()
            .and_then(|a| a.failed.clone())
            .filter(|_| self.channel.is_none())
        {
            say(failed, &theme)
        } else if self.channel.is_none() {
            let words = match (self.account(), problem) {
                (Some(a), _) if a.reading => "Reading the channels…",
                (_, Some(_)) => "The channels could not be read.",
                _ => "This account has no channels.",
            };
            say(words.into(), &theme)
        } else {
            match (&self.view, &self.history_load, &self.thread_load) {
                (View::Channel, Load::Loading, _) => say("Reading the messages…".into(), &theme),
                (View::Channel, Load::Failed(why), _) | (View::Thread(_), _, Load::Failed(why)) => {
                    say(why.clone(), &theme)
                }
                (View::Thread(_), _, Load::Loading) => say("Reading the thread…".into(), &theme),
                _ => self.messages(cx),
            }
        };
        let notice = problem.and_then(|problem| {
            let pane = pane.clone();
            banner(
                problem,
                move |_, cx| pane.update(cx, |p, cx| p.reload(cx)),
                &theme,
            )
        });
        let composer = self.composer_shown().then(|| {
            div()
                .id("messages-composer")
                .debug_selector(|| "messages-composer".into())
                .flex_none()
                .px(px(12.))
                .pb(px(12.))
                .child(self.composer_now().clone())
        });
        div()
            .id("messages-pane")
            .debug_selector(|| "messages-pane".into())
            .key_context("Messages")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && matches!(this.view, View::Thread(_)) {
                    this.back(cx);
                }
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .rounded(radius::lg())
            .bg(theme.card)
            .child(header)
            .when_some(self.said.clone(), |d, said| {
                d.child(
                    div()
                        .debug_selector(|| "messages-said".into())
                        .px(px(12.))
                        .pb(px(8.))
                        .text_size(TextSize::Xs.font_size())
                        .text_color(theme.danger)
                        .child(said),
                )
            })
            .children(notice)
            .child(div().flex_1().min_h_0().child(body))
            .children(composer)
    }
}

impl MessagesPane {
    /// The messages of the open channel or thread, as a list that lays out the rows on screen. In a channel with an older page,
    /// the first row is the one that reads it.
    fn messages(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let (lines, state, link, head): (
            Rc<Vec<Line>>,
            ListState,
            Option<OpenThread>,
            Option<bool>,
        ) = match &self.view {
            View::Channel => {
                let weak = cx.entity().downgrade();
                let open: OpenThread = Rc::new(move |root, _, cx| {
                    drop(weak.update(cx, |p, cx| p.open_thread(root, cx)))
                });
                (
                    self.history.clone(),
                    self.list.clone(),
                    self.threads().then_some(open),
                    self.head.then_some(self.loading_older),
                )
            }
            View::Thread(_) => (self.thread.clone(), self.thread_list.clone(), None, None),
        };
        if lines.is_empty() {
            let words = if self.effective_problem().is_some() {
                "The messages could not be read."
            } else {
                "No messages yet."
            };
            return say(words.into(), cx.theme());
        }
        let pane = cx.entity();
        let first = usize::from(head.is_some());
        list(state, move |ix, _, cx| {
            match (head, ix.checked_sub(first)) {
                (Some(loading), None) => {
                    let pane = pane.clone();
                    load_older(loading, move |_, cx| {
                        pane.update(cx, |p, cx| p.load_older(cx))
                    })
                }
                (_, Some(at)) => match lines.get(at) {
                    Some(line) => draw_line(at, line, link.clone(), cx),
                    None => div().into_any_element(),
                },
                (None, None) => div().into_any_element(),
            }
        })
        .size_full()
        .into_any_element()
    }
}
