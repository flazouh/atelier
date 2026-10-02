use std::sync::Arc;

use atelier_ui::{
    ActiveTheme,
    PrCard,
    PrChipData,
    button::{Button, ButtonVariant},
    merge::{Action, Choice, MergeFacts},
    typography::TextSize,
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div,
};
use atelier_ui::scale::px;
use atelier_forge::{Check, Forge, Pull, PullRef, PullUpdate, present};
use atelier_pr_view::actions::{Ask, ask};

use super::watch::PullWatch;
use super::types::CardEvent;
use super::helpers::verb;

impl EventEmitter<CardEvent> for PullCard {}

pub struct PullCard {
    pub(super) reference: PullRef,
    pub(super) forge: Arc<dyn Forge>,
    pub(super) watch: Entity<PullWatch>,
    /// What the last write said: a refusal in the forge's words, or what was done.
    pub said: Option<SharedString>,
    /// The action waiting for the reader's yes.
    pub confirm: Option<Action>,
    choice: Option<Choice>,
    pub busy: bool,
    writing: Task<()>,
    _watch: Subscription,
}

impl PullCard {
    pub fn new(reference: PullRef, forge: Arc<dyn Forge>, cx: &mut Context<Self>) -> Self {
        let watch = super::watch::watch(reference.clone(), forge.clone(), cx);
        let _watch = cx.observe(&watch, |_, _, cx| cx.notify());
        Self { reference, forge, watch, said: None, confirm: None, choice: None, busy: false, writing: Task::ready(()), _watch }
    }

    pub fn reference(&self) -> &PullRef {
        &self.reference
    }

    pub fn pull<'a>(&self, cx: &'a App) -> Option<&'a Pull> {
        self.watch.read(cx).pull.as_ref()
    }

    pub fn checks<'a>(&self, cx: &'a App) -> &'a [Check] {
        &self.watch.read(cx).checks
    }

    /// Why the last read failed, while reads fail.
    #[cfg(test)]
    pub fn unread(&self, cx: &App) -> Option<SharedString> {
        self.watch.read(cx).unread.clone()
    }

    /// The wait before the next read; `None` when the card reads no more on its own.
    #[cfg(test)]
    pub fn next(&self, cx: &App) -> Option<std::time::Duration> {
        self.watch.read(cx).next
    }

    /// The facts the merge button stands on.
    pub fn facts(&self, cx: &App) -> Option<MergeFacts> {
        self.pull(cx).map(|p| present::merge_facts(p, Some(self.checks(cx)), &[]))
    }

    /// The reader's merge choice, else the repository's defaults.
    fn choice(&self, cx: &App) -> Option<Choice> {
        self.choice.or_else(|| self.facts(cx).map(|f| Choice { method: f.default_method, auto: false, delete_branch: f.delete_branch }))
    }

    pub fn set_choice(&mut self, choice: Choice, cx: &mut Context<Self>) {
        self.choice = Some(choice);
        cx.notify();
    }

    /// A press of a merge action: a merge, a delete or a revert waits for a yes, the rest runs.
    pub fn press(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if matches!(action, Action::Merge(_) | Action::BypassAndMerge(_) | Action::MergeWhenReady(_) | Action::DeleteBranch | Action::Revert) {
            self.confirm = Some(action);
            return cx.notify();
        }
        self.run(action, cx);
    }

    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(action) = self.confirm.take() {
            self.run(action, cx);
        }
    }

    pub fn cancel_confirm(&mut self, cx: &mut Context<Self>) {
        self.confirm = None;
        cx.notify();
    }

    /// The question the confirmation asks, in the reader's terms.
    pub fn confirm_words(&self, cx: &App) -> Option<String> {
        let (action, pull) = (self.confirm?, self.pull(cx)?);
        let number = pull.reference.number;
        let deletes = self.choice(cx).is_some_and(|c| c.delete_branch);
        let and_delete = if deletes { format!(", and delete {}", pull.head) } else { String::new() };
        Some(match action {
            Action::Merge(m) | Action::BypassAndMerge(m) => format!("{} #{number} into {}{and_delete}?", verb(m), pull.base),
            Action::MergeWhenReady(m) => format!("{} #{number} into {} once it is ready{and_delete}?", verb(m), pull.base),
            Action::DeleteBranch => format!("Delete the branch {}?", pull.head),
            Action::Revert => format!("Open a pull request that reverts #{number}?"),
            _ => return None,
        })
    }

    pub(super) fn run(&mut self, action: Action, cx: &mut Context<Self>) {
        let (Some(pull), Some(choice)) = (self.pull(cx).cloned(), self.choice(cx)) else { return };
        let title = format!("{} (#{})", pull.title, pull.reference.number);
        let request = ask(action, choice, &title, &pull.body, &pull.head_sha);
        let done = match &request {
            Ask::Merge(_) => format!("Merged #{}", pull.reference.number),
            Ask::DeleteBranch => format!("Deleted {}", pull.head),
            Ask::Revert => format!("Opened a pull request that reverts #{}", pull.reference.number),
            _ => String::new(),
        };
        self.busy = true;
        self.said = None;
        let (forge, reference) = (self.forge.clone(), self.reference.clone());
        let writing = cx.background_spawn(async move {
            match request {
                Ask::Merge(request) => forge.merge(&reference, &request).map(drop),
                Ask::Ready => forge.update_pull(&reference, &PullUpdate { ready: Some(true), ..PullUpdate::default() }),
                Ask::UpdateBranch { method, expected_head } => forge.update_branch(&reference, method, &expected_head),
                Ask::CancelAutoMerge => forge.cancel_auto_merge(&reference),
                Ask::Dequeue => forge.dequeue(&reference),
                Ask::DeleteBranch => forge.delete_branch(&reference),
                Ask::Revert => forge.revert(&reference).map(drop),
            }
        });
        self.writing = cx.spawn(async move |this, cx| {
            let result = writing.await;
            _ = this.update(cx, |card, cx| {
                card.busy = false;
                card.said = match result {
                    Ok(()) => (!done.is_empty()).then(|| done.into()),
                    Err(error) => Some(error.to_string().into()),
                };
                card.watch.update(cx, |w, cx| w.read_now(cx));
                cx.notify();
            });
        });
        cx.notify();
    }
}

impl Render for PullCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Drawn: the watch reads on while the card is on screen in an active window.
        let active = window.is_window_active();
        self.watch.update(cx, |w, cx| w.drawn(active, cx));
        let theme = cx.theme().clone();
        let unread = self.watch.read(cx).unread.clone();
        let words = |text: SharedString, danger: bool| {
            div().text_size(TextSize::Xs.font_size()).text_color(if danger { theme.danger } else { theme.muted_foreground }).child(text)
        };
        let Some(pull) = self.pull(cx).cloned() else {
            let failed = unread.is_some();
            let text = unread.unwrap_or_else(|| format!("Reading #{}…", self.reference.number).into());
            return div().px(px(4.)).child(words(text, failed)).into_any_element();
        };
        let chip = PrChipData {
            number: pull.reference.number,
            repo: pull.reference.repo.slug().into(),
            title: pull.title.clone().into(),
            state: present::pr_state(pull.state),
            url: pull.url.clone().into(),
        };
        let this = cx.entity().downgrade();
        let (presses, chooses, opens) = (this.clone(), this.clone(), this.clone());
        let mut card = PrCard::new(format!("pull-card-{}", pull.reference.number), chip)
            .checks(present::checks(pull.checks))
            .review(present::review_state(pull.review))
            .on_merge(move |action, _, cx| drop(presses.update(cx, |c, cx| c.press(action, cx))))
            .on_merge_choice(move |choice, _, cx| drop(chooses.update(cx, |c, cx| c.set_choice(choice, cx))))
            .on_open(move |_, _, cx| {
                _ = opens.update(cx, |c, cx| cx.emit(CardEvent::Show(c.reference.clone())));
            });
        if let (Some(facts), Some(choice)) = (self.facts(cx), self.choice(cx)) {
            card = card.merge(facts, choice);
        }
        let confirm = self.confirm_words(cx).map(|question| {
            let (no, yes) = (this.clone(), this.clone());
            let label = match self.confirm {
                Some(Action::DeleteBranch) => "Delete branch",
                Some(Action::Revert) => "Open the revert",
                Some(Action::Merge(m) | Action::BypassAndMerge(m) | Action::MergeWhenReady(m)) => verb(m),
                _ => "Go on",
            };
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(4.))
                .child(div().flex_1().min_w_0().child(words(question.into(), false)))
                .child(Button::new("pull-card-no").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                    _ = no.update(cx, |c, cx| c.cancel_confirm(cx));
                }))
                .child(Button::new("pull-card-yes").label(label).variant(ButtonVariant::Primary).on_click(move |_, _, cx| {
                    _ = yes.update(cx, |c, cx| c.confirm(cx));
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(card)
            .children(confirm)
            .children(self.said.clone().map(|w| words(w, self.said_is_refusal())))
            .children(unread.map(|w| words(w, true)))
            .into_any_element()
    }
}

impl PullCard {
    /// Whether the last words are a refusal, not a report of what was done.
    fn said_is_refusal(&self) -> bool {
        self.said.as_deref().is_some_and(|w| !(w.starts_with("Merged") || w.starts_with("Deleted") || w.starts_with("Opened")))
    }
}
