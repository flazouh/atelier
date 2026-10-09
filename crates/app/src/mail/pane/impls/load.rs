use std::sync::Arc;

use atelier_capabilities::{
    CapError, CapResult, Ref,
    mail::{MailOperation, MailProvider, Role, Thread, ThreadSummary},
};
use gpui_kit::{AppContext, Context, Window};

use super::super::{
    helpers::{apply, new_list, react, read_boxes, read_more, read_thread, read_threads, set_tail},
    structs::{Account, Boxes, Fetched, MailPane},
    types::{Load, POLL, Reaction},
};
use crate::mail::{
    map::{message_view_of, thread_row_of},
    source::MailSource,
};

impl MailPane {
    /// Gives the pane the app's mail providers. The same ones as before change nothing; others replace the accounts, and the
    /// mailboxes of each are read, off the UI thread.
    pub fn set_providers(&mut self, providers: Vec<Arc<dyn MailProvider>>, cx: &mut Context<Self>) {
        if self.source.same_as(&providers) {
            return;
        }
        self.epoch += 1;
        self.source = MailSource::from_providers(providers);
        self.accounts = self
            .source
            .accounts()
            .into_iter()
            .map(|(choice, provider)| Account::new(choice, provider))
            .collect();
        self.listening.clear();
        self.held.clear();
        (self.shown, self.mailbox) = (0, None);
        self.clear_list();
        self.clear_thread();
        for at in 0..self.accounts.len() {
            self.read_boxes(at, cx);
            self.listen(at, cx);
        }
        self.start_polling(cx);
        cx.notify();
    }

    /// Forgets the threads of the open mailbox, and waits for a new reading.
    fn clear_list(&mut self) {
        self.list_turn += 1;
        self.threads = Default::default();
        self.list = new_list();
        (self.next, self.tail, self.pages, self.loading_more) = (None, false, 1, false);
        self.threads_load = Load::Loading;
        (self.problem, self.said, self.said_reading) = (None, None, false);
    }

    /// Forgets the open thread. What the reader wrote for it is kept first.
    pub(in crate::mail::pane) fn clear_thread(&mut self) {
        self.thread_turn += 1;
        self.open = None;
        self.thread = None;
        self.views = Default::default();
        self.thread_load = Load::Loading;
        (self.whole, self.link, self.menu, self.draft) = (Vec::new(), None, None, None);
    }

    /// Reads the mailboxes of account `at` again, with what it can do, off the UI thread.
    pub(in crate::mail::pane) fn read_boxes(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(provider) = self.accounts.get(at).map(|a| a.provider.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let reading = cx.background_spawn(async move { read_boxes(provider.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.boxes_read(at, result, epoch, cx))
                .ok();
        })
        .detach();
    }

    fn boxes_read(
        &mut self,
        at: usize,
        result: CapResult<Boxes>,
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
            Ok(Boxes { caps, rows }) => {
                (account.caps, account.boxes) = (caps, rows);
                (account.problem, account.failed) = (None, None);
            }
            Err(error) => match react(&error) {
                Reaction::Raise(problem) => account.problem = Some(problem),
                Reaction::Hide | Reaction::Line(_) => {
                    account.failed = Some(error.to_string().into())
                }
            },
        }
        // The first mailbox of the first account that has one opens (the inbox comes first), so the pane has something to show.
        if self.mailbox.is_none()
            && let Some((first, mailbox)) = self
                .accounts
                .iter()
                .enumerate()
                .find_map(|(i, a)| a.boxes.first().map(|b| (i, b.reference.clone())))
        {
            self.enter_mailbox(first, &mailbox, cx);
        }
        cx.notify();
    }

    /// Shows what a reference names, in the account it names: a mailbox with its threads, or a thread, with the inbox of the
    /// account (or its first mailbox) behind it when no mailbox of the account is open. Any other reference shows nothing.
    pub fn open_ref(&mut self, reference: &Ref, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self
            .accounts
            .iter()
            .position(|a| a.choice.provider == reference.provider && a.choice.account == reference.account)
        else {
            return;
        };
        if reference.id.starts_with("b:") {
            return self.open_mailbox(at, reference, window, cx);
        }
        if !reference.id.starts_with("t:") {
            return;
        }
        if self.shown != at || self.mailbox.is_none() {
            let boxes = &self.accounts[at].boxes;
            let behind = boxes.iter().find(|b| b.role == Role::Inbox).or(boxes.first()).map(|b| b.reference.clone());
            match behind {
                Some(mailbox) => self.open_mailbox(at, &mailbox, window, cx),
                None => self.shown = at,
            }
        }
        self.open_thread(reference, window, cx);
    }

    /// Shows the threads of `mailbox` of account `at`, from the newest.
    pub fn open_mailbox(
        &mut self,
        at: usize,
        mailbox: &Ref,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if at >= self.accounts.len() || (self.shown == at && self.mailbox.as_ref() == Some(mailbox))
        {
            return;
        }
        // A search belongs to the mailbox it was typed over.
        self.search.update(cx, |s, cx| s.set_value("", window, cx));
        self.enter_mailbox(at, mailbox, cx);
    }

    fn enter_mailbox(&mut self, at: usize, mailbox: &Ref, cx: &mut Context<Self>) {
        self.stash(cx);
        (self.shown, self.mailbox) = (at, Some(mailbox.clone()));
        self.query.clear();
        self.clear_list();
        self.clear_thread();
        self.read_threads(cx);
        cx.notify();
    }

    /// The reader sent the words of the search box with Enter: the mailbox is read again for them.
    pub(in crate::mail::pane) fn search_typed(&mut self, cx: &mut Context<Self>) {
        let text = self.search.read(cx).value().trim().to_string();
        if self.mailbox.is_none() || text == self.query {
            return;
        }
        self.query = text;
        self.clear_list();
        self.read_threads(cx);
        cx.notify();
    }

    /// Reads the open mailbox's first pages again (as many as are read), off the UI thread.
    pub(in crate::mail::pane) fn read_threads(&mut self, cx: &mut Context<Self>) {
        let (Some(account), Some(mailbox)) = (self.account(), self.mailbox.clone()) else {
            return;
        };
        if !account.caps.can(MailOperation::Search) {
            self.threads_load = Load::Failed("This account cannot list its mail.".into());
            return;
        }
        let (provider, pages, text, turn) = (
            account.provider.clone(),
            self.pages,
            self.query.clone(),
            self.list_turn,
        );
        let reading = cx.background_spawn(async move {
            read_threads(
                provider.as_ref(),
                &mailbox,
                &text,
                pages,
                crate::agent_session::now(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.threads_read(result, turn, cx))
                .ok();
        })
        .detach();
    }

    fn threads_read(&mut self, result: CapResult<Fetched>, turn: u64, cx: &mut Context<Self>) {
        if turn != self.list_turn {
            return;
        }
        match result {
            Ok(Fetched { rows, next }) => {
                apply(&mut self.threads, &self.list, rows);
                self.next = next;
                set_tail(
                    &self.list,
                    &mut self.tail,
                    self.threads.len(),
                    self.next.is_some(),
                );
                self.threads_load = Load::Ready;
                self.good_reading();
            }
            Err(error) => self.fail(error, MailOperation::Search, "Could not read the mail", cx),
        }
        cx.notify();
    }

    pub(in crate::mail::pane) fn good_reading(&mut self) {
        self.problem = None;
        if std::mem::take(&mut self.said_reading) {
            self.said = None;
        }
    }

    /// Reads the page after the last one shown, when there is one, and puts it below.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        let (Some(provider), Some(mailbox), Some(cursor)) = (
            self.account().map(|a| a.provider.clone()),
            self.mailbox.clone(),
            self.next.clone(),
        ) else {
            return;
        };
        if self.loading_more {
            return;
        }
        self.loading_more = true;
        let (text, turn) = (self.query.clone(), self.list_turn);
        let reading = cx.background_spawn(async move {
            read_more(
                provider.as_ref(),
                &mailbox,
                &text,
                &cursor,
                crate::agent_session::now(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.more_read(result, turn, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn more_read(&mut self, result: CapResult<Fetched>, turn: u64, cx: &mut Context<Self>) {
        if turn != self.list_turn {
            return;
        }
        self.loading_more = false;
        match result {
            Ok(Fetched { rows, next }) => {
                // A thread may sit in two pages when the mailbox moved between the readings.
                let mut all: Vec<_> = self.threads.iter().cloned().collect();
                let held: Vec<Ref> = all.iter().map(|r| r.reference.clone()).collect();
                all.extend(rows.into_iter().filter(|r| !held.contains(&r.reference)));
                apply(&mut self.threads, &self.list, all);
                self.next = next;
                set_tail(
                    &self.list,
                    &mut self.tail,
                    self.threads.len(),
                    self.next.is_some(),
                );
                self.pages += 1;
            }
            Err(error) => self.fail(error, MailOperation::Search, "Could not read more mail", cx),
        }
        cx.notify();
    }

    /// Shows `thread` in the reading pane, and reads it. Opening it marks nothing read: that is the reader's choice, or the
    /// provider's own doing, which the reading brings back.
    pub fn open_thread(&mut self, thread: &Ref, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.as_ref() == Some(thread) {
            return;
        }
        self.stash(cx);
        self.clear_thread();
        self.messages_scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)));
        self.open = Some(thread.clone());
        (self.said, self.said_reading) = (None, false);
        let kept = self.held.iter().position(|h| &h.thread == thread);
        let kept = kept.map(|at| self.held.remove(at));
        self.draft = kept.as_ref().and_then(|k| k.draft.clone());
        let text = kept.map(|k| k.text).unwrap_or_default();
        self.compose
            .update(cx, |c, cx| c.set_value(text, window, cx));
        self.read_open(cx);
        cx.notify();
    }

    /// Closes the open thread, keeping the reader's words for it.
    pub fn close_thread(&mut self, cx: &mut Context<Self>) {
        self.stash(cx);
        self.clear_thread();
        cx.notify();
    }

    /// Keeps the draft and the words of the open thread, so a visit to another thread loses nothing.
    pub(in crate::mail::pane) fn stash(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.open.clone() else {
            return;
        };
        let text = self.compose.read(cx).value();
        self.held.retain(|h| h.thread != thread);
        if self.draft.is_some() || !text.trim().is_empty() {
            self.held.push(super::super::structs::Held {
                thread,
                draft: self.draft.clone(),
                text,
            });
        }
    }

    pub(in crate::mail::pane) fn read_open(&mut self, cx: &mut Context<Self>) {
        let (Some(account), Some(thread)) = (self.account(), self.open.clone()) else {
            return;
        };
        let (provider, turn) = (account.provider.clone(), self.thread_turn);
        let reading = cx.background_spawn(async move { read_thread(provider.as_ref(), &thread) });
        cx.spawn(async move |this, cx| {
            let result = reading.await;
            this.update(cx, |this, cx| this.thread_read(result, turn, cx))
                .ok();
        })
        .detach();
    }

    fn thread_read(&mut self, result: CapResult<Thread>, turn: u64, cx: &mut Context<Self>) {
        if turn != self.thread_turn {
            return;
        }
        match result {
            Ok(thread) => {
                let me = self
                    .account()
                    .map(|a| a.choice.account.clone())
                    .unwrap_or_default();
                self.views = std::rc::Rc::new(
                    thread
                        .messages
                        .iter()
                        .map(|m| message_view_of(m, &me))
                        .collect(),
                );
                self.sync_row(&thread.summary, cx);
                self.thread = Some(thread);
                self.thread_load = Load::Ready;
                if std::mem::take(&mut self.to_end) {
                    self.messages_scroll.scroll_to_bottom();
                }
                self.good_reading();
            }
            Err(error) => self.fail(
                error,
                MailOperation::Thread,
                "Could not read the thread",
                cx,
            ),
        }
        cx.notify();
    }

    /// The thread as the provider has just given it replaces the thread as the list knew it. A provider that marks a thread
    /// read by giving it says so here, and the mailboxes' counts are read again for it.
    fn sync_row(&mut self, summary: &ThreadSummary, cx: &mut Context<Self>) {
        let me = self
            .account()
            .map(|a| a.choice.account.clone())
            .unwrap_or_default();
        let row = thread_row_of(summary, &me, crate::agent_session::now());
        let Some(at) = self
            .threads
            .iter()
            .position(|r| r.reference == row.reference)
        else {
            return;
        };
        if self.threads[at] == row {
            return;
        }
        let unread_changed = self.threads[at].unread != row.unread;
        let mut next: Vec<_> = self.threads.iter().cloned().collect();
        next[at] = row;
        apply(&mut self.threads, &self.list, next);
        if unread_changed {
            self.read_boxes(self.shown, cx);
        }
    }

    /// Reads everything shown again: the mailboxes of the account, the threads, and the open thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.read_boxes(self.shown, cx);
        self.read_threads(cx);
        self.read_open(cx);
    }

    /// Answers an error of a call as [`react`] says. `doing` starts the line that says it.
    pub(in crate::mail::pane) fn fail(
        &mut self,
        error: CapError,
        operation: MailOperation,
        doing: &str,
        cx: &mut Context<Self>,
    ) {
        let words = error.to_string();
        let loading = |load: &Load| matches!(load, Load::Loading);
        let reading = matches!(operation, MailOperation::Search | MailOperation::Thread);
        let mut says = false;
        match react(&error) {
            Reaction::Raise(problem) => {
                self.problem = Some(problem);
                // The banner sits over the card, so a first reading that failed shows it over the empty list.
                match operation {
                    MailOperation::Search if loading(&self.threads_load) => {
                        self.threads_load = Load::Ready
                    }
                    MailOperation::Thread if loading(&self.thread_load) => {
                        self.thread_load = Load::Ready
                    }
                    _ => {}
                }
            }
            // The control of a call the provider does not offer goes. A reading it cannot give is a plain failure.
            Reaction::Hide if !reading => {
                if let Some(account) = self.accounts.get_mut(self.shown) {
                    account.caps.operations.retain(|o| *o != operation);
                }
            }
            Reaction::Hide | Reaction::Line(_) => {
                // A first reading that failed takes the card, in words; any other failure is a line under the header.
                let first = match operation {
                    MailOperation::Search if loading(&self.threads_load) => {
                        Some(&mut self.threads_load)
                    }
                    MailOperation::Thread if loading(&self.thread_load) => {
                        Some(&mut self.thread_load)
                    }
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
            self.said_reading = reading;
        }
        cx.notify();
    }

    /// Asks account `at` for what happens from now on, off the UI thread.
    fn listen(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(provider) = self.accounts.get(at).map(|a| a.provider.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let asking = cx.background_spawn(async move { provider.subscribe() });
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

    /// Looks at what the subscriptions have told. A burst is one reading: of the threads and the open thread when the shown
    /// account changed, and of the mailboxes of each account that told anything (their counts).
    fn poll(&mut self, cx: &mut Context<Self>) {
        let (mut here, mut told_by) = (false, std::collections::BTreeSet::new());
        let mut open_changed = false;
        let open = self.open.clone();
        let shown = self.shown;
        self.listening.retain_mut(|(at, told)| {
            loop {
                match told.try_recv() {
                    Ok(event) => {
                        told_by.insert(*at);
                        if *at == shown {
                            here = true;
                            open_changed |= open.as_ref() == Some(&event.thread.reference);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
                }
            }
        });
        if here {
            self.read_threads(cx);
        }
        if open_changed {
            self.read_open(cx);
        }
        for at in told_by {
            self.read_boxes(at, cx);
        }
    }
}
