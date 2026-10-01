//! The pull requests list on the screen: the reader's working set as Courts, drawn from the disk at once,
//! read again from the forge in the background, and kept current with a backoff. It opens nothing itself:
//! a row reports [`ListEvent::Open`] and the owner decides where the pull request shows.
use std::sync::Arc;

use beui::{ActiveTheme, Button, ButtonSize, ButtonVariant, CourtList};
use futures_channel::mpsc;
use futures_util::StreamExt;
use gpui_kit::{
    AppContext, Context, EventEmitter, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window, div,
    prelude::FluentBuilder, px,
};
use atelier_forge::{Forge, ForgeError, Involved, PullRef};

use crate::{
    list::ListModel,
    services::{Services, now},
    sync::Cadence,
};

/// What the list tells its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListEvent {
    Open(PullRef),
}

enum Msg {
    Cached(Option<(Vec<Involved>, u64)>, std::collections::HashMap<(String, u64), u64>),
    Read(Result<Vec<Involved>, ForgeError>),
}

pub struct PullList {
    services: Arc<Services>,
    model: ListModel,
    cadence: Cadence,
    /// Why refreshing stopped for good (a sign-in is missing), or `None`.
    stopped: bool,
    reading: bool,
    tx: mpsc::UnboundedSender<Msg>,
    _pump: Task<()>,
    _loop: Task<()>,
}

impl EventEmitter<ListEvent> for PullList {}

impl PullList {
    pub fn new(services: Arc<Services>, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = mpsc::unbounded::<Msg>();
        let pump = cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await {
                if this.update(cx, |list, cx| list.take(msg, cx)).is_err() {
                    return;
                }
            }
        });
        let mut list = Self {
            cadence: Cadence::new(services.config.list_refresh),
            services,
            model: ListModel::default(),
            stopped: false,
            reading: false,
            tx,
            _pump: pump,
            _loop: Task::ready(()),
        };
        list.start(cx);
        list
    }

    pub fn model(&self) -> &ListModel {
        &self.model
    }

    /// The disk first, then the forge; after that, on the cadence.
    fn start(&mut self, cx: &mut Context<Self>) {
        let (services, tx) = (self.services.clone(), self.tx.clone());
        cx.background_spawn(async move {
            let opened = services.reviewed.opened_at().unwrap_or_default();
            let _ = tx.unbounded_send(Msg::Cached(services.list_snapshot.load(), opened));
        })
        .detach();
        self.read(cx);
        self._loop = cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |list, _| list.wait()) else { return };
                let Some(wait) = wait else {
                    // Asking cannot help until the reader acts; check back for a retry request.
                    cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                    continue;
                };
                cx.background_executor().timer(wait).await;
                if this.update(cx, |list, cx| list.read(cx)).is_err() {
                    return;
                }
            }
        });
    }

    fn wait(&self) -> Option<std::time::Duration> {
        (!self.stopped).then_some(self.services.config.list_refresh)
    }

    /// Reads the working set in the background.
    pub fn read(&mut self, cx: &mut Context<Self>) {
        if self.reading {
            return;
        }
        self.reading = true;
        let (forge, tx, scope) = (self.services.forge.clone(), self.tx.clone(), self.services.config.repo.clone());
        cx.background_spawn(async move {
            let _ = tx.unbounded_send(Msg::Read(read_involved(forge.as_ref(), scope.as_ref())));
        })
        .detach();
        cx.notify();
    }

    /// The reader asked again, after a failure that stopped the asking.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.stopped = false;
        self.read(cx);
    }

    fn take(&mut self, msg: Msg, cx: &mut Context<Self>) {
        match msg {
            Msg::Cached(cached, opened) => {
                self.model.set_opened(opened);
                // A cached list draws only until the forge has answered.
                if let Some((items, at)) = cached.filter(|_| !self.model.loaded) {
                    self.model.set(items, at);
                }
            }
            Msg::Read(Ok(items)) => {
                self.reading = false;
                let at = now();
                self.cadence.after_answer(true);
                self.model.error = None;
                let (services, saved) = (self.services.clone(), items.clone());
                cx.background_spawn(async move {
                    let _ = services.list_snapshot.save(&saved, at);
                })
                .detach();
                self.model.set(items, at);
            }
            Msg::Read(Err(error)) => {
                self.reading = false;
                self.stopped = self.cadence.after_failure(&error).is_none();
                self.model.error = Some(error);
            }
        }
        cx.notify();
    }

    /// The reader opened a pull request: it is no longer unread.
    pub fn opened(&mut self, reference: &PullRef, cx: &mut Context<Self>) {
        let at = now();
        self.model.mark_opened(reference, at);
        let (services, reference) = (self.services.clone(), reference.clone());
        cx.background_spawn(async move {
            let _ = services.reviewed.opened(&reference, at);
        })
        .detach();
        cx.notify();
    }
}

fn read_involved(forge: &dyn Forge, scope: Option<&atelier_forge::RepoRef>) -> Result<Vec<Involved>, ForgeError> {
    match scope {
        Some(repo) => forge.involved_in(repo),
        None => forge.involved(),
    }
}

impl Render for PullList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let this = cx.entity().downgrade();
        let rows = self.model.rows(now());
        let status = match (&self.model.error, self.reading, self.model.loaded) {
            (Some(error), _, _) => Some(error.to_string()),
            (None, true, false) => Some("Reading your pull requests…".to_string()),
            _ => None,
        };
        let stopped = self.stopped;
        let retry = cx.listener(|list, _, _, cx| list.retry(cx));
        div()
            .id("pull-request-list")
            .size_full()
            .overflow_y_scroll()
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(8.))
                    .h(px(32.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child("Pull requests"))
                    .child(div().flex_1())
                    .when_some(status, |d, status| d.child(div().text_size(px(12.)).text_color(if self.model.error.is_some() { theme.danger } else { muted }).child(status)))
                    .when(stopped, |d| d.child(Button::new("retry-list").label("Try again").size(ButtonSize::Sm).variant(ButtonVariant::Secondary).on_click(move |e, w, cx| retry(e, w, cx)))),
            )
            .when(self.model.loaded && rows.is_empty(), |d| d.child(div().px(px(12.)).py(px(24.)).text_color(muted).child("Nothing needs you, and nothing waits on anyone.")))
            .when(!rows.is_empty(), |d| {
                d.child(CourtList::new("courts", rows).on_open(move |chip, _, cx| {
                    this.update(cx, |list, cx| {
                        if let Some(reference) = list.model.reference_of(chip) {
                            cx.emit(ListEvent::Open(reference));
                        }
                    })
                    .ok();
                }))
            })
    }
}
