//! The "Merge" story: the merge button in every state, the PR card with its button, and the merge box.
//!
//! The story is the owner the components report to. It keeps the reader's last method for the
//! repository, as the app will, so a method picked on one button is the method on every button after
//! it; it prints each press and shows the last one under the title.

use std::collections::HashMap;

use beui::{
    MergeBox, MergeBoxEvent, MergeButton, PrCard, PrChipData, PrState,
    merge::{Action, Choice, MergeFacts, MergeMethod, PullState, Queue, ReviewNeed, Rights, UpdateWay, first_choice},
    pr::{Checks, ReviewState},
    theme::ActiveTheme,
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window, div, px,
};

use crate::{agent_parts::pr_3344, narrow, section};

const REPO: &str = "flazouh/lathe";

/// Each state the button shows, with its name in the story.
pub fn states() -> Vec<(&'static str, MergeFacts)> {
    let open = MergeFacts { default_method: MergeMethod::Squash, delete_branch: true, auto_merge: Some(false), ..MergeFacts::default() };
    vec![
        ("Ready", open.clone()),
        ("Draft", MergeFacts { draft: true, ..open.clone() }),
        ("Conflicts", MergeFacts { conflicts: vec!["src/request.rs".into(), "src/relay.rs".into()], ..open.clone() }),
        ("Behind its base", MergeFacts { behind: Some(vec![UpdateWay::Merge, UpdateWay::Rebase]), ..open.clone() }),
        ("Checks failing", MergeFacts { checks_failing: 1, ..open.clone() }),
        ("Checks running", MergeFacts { checks_running: 2, ..open.clone() }),
        ("Review missing", MergeFacts { review: ReviewNeed::Missing, ..open.clone() }),
        ("Changes asked", MergeFacts { review: ReviewNeed::ChangesAsked(vec!["Ada".into()]), ..open.clone() }),
        ("Merge when ready on", MergeFacts { checks_running: 2, auto_merge: Some(true), ..open.clone() }),
        ("Merge queue", MergeFacts { queue: Some(Queue { queued: false, position: None }), ..open.clone() }),
        ("In the queue", MergeFacts { queue: Some(Queue { queued: true, position: Some(3) }), ..open.clone() }),
        ("Admin, checks failing", MergeFacts { checks_failing: 1, rights: Rights::Bypass, ..open.clone() }),
        ("Cannot merge", MergeFacts { rights: Rights::Cannot, ..open.clone() }),
        ("Merged", MergeFacts { state: PullState::Merged, ..open }),
    ]
}

pub struct MergeStory {
    states: Vec<(&'static str, MergeFacts)>,
    /// The reader's last method in each repository.
    remembered: HashMap<SharedString, MergeMethod>,
    /// The toggles each button holds; the method comes from `remembered`.
    choices: Vec<Choice>,
    /// The last press, as the story heard it.
    last: Option<SharedString>,
    ready: Entity<MergeBox>,
    blocked: Entity<MergeBox>,
    merged: Entity<MergeBox>,
    _boxes: [Subscription; 3],
}

const BODY: &str = "Before this, a client that aborted between two chunks left the relay writing into a closed sink.\n\nThe stream detaches on abort, and the second write is a no-op.";

impl MergeStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let states = states();
        let choices = states.iter().map(|(_, facts)| first_choice(facts, None)).collect();
        let title = pr_3344().title;
        let boxed = |facts: MergeFacts, window: &mut Window, cx: &mut Context<Self>| {
            let choice = first_choice(&facts, None);
            let title = title.clone();
            cx.new(|cx| MergeBox::new(facts, choice, title, BODY, window, cx))
        };
        let ready = boxed(states[0].1.clone(), window, cx);
        let blocked = boxed(
            MergeFacts { checks_running: 2, review: ReviewNeed::ChangesAsked(vec!["Ada".into()]), ..states[0].1.clone() },
            window,
            cx,
        );
        let merged = boxed(states[0].1.clone(), window, cx);
        merged.update(cx, |b, cx| b.merged(false, cx));
        let _boxes = [&ready, &blocked, &merged].map(|b| cx.subscribe_in(b, window, Self::heard));
        Self { states, remembered: HashMap::new(), choices, last: None, ready, blocked, merged, _boxes }
    }

    fn heard(&mut self, merge_box: &Entity<MergeBox>, event: &MergeBoxEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event {
            MergeBoxEvent::Act { action, choice, title, .. } => {
                self.report(format!("{} \"{title}\"", action.word()), cx);
                let deleted = match action {
                    Action::Merge(_) | Action::BypassAndMerge(_) => choice.delete_branch,
                    Action::DeleteBranch => true,
                    _ => return,
                };
                merge_box.update(cx, |b, cx| b.merged(deleted, cx));
            }
            MergeBoxEvent::Chose(choice) => self.remember(choice.method, cx),
        }
    }

    fn report(&mut self, line: String, cx: &mut Context<Self>) {
        println!("merge: {line}");
        self.last = Some(line.into());
        cx.notify();
    }

    fn remember(&mut self, method: MergeMethod, cx: &mut Context<Self>) {
        self.remembered.insert(REPO.into(), method);
        cx.notify();
    }

    /// The choice for button `i`: its own toggles, with the remembered method when the repository allows it.
    fn choice(&self, i: usize) -> Choice {
        let facts = &self.states[i].1;
        let method = first_choice(facts, self.remembered.get(REPO).copied()).method;
        Choice { method, ..self.choices[i] }
    }

    fn button(&self, i: usize, id: impl Into<gpui_kit::ElementId>, compact: bool, cx: &mut Context<Self>) -> MergeButton {
        let (acted, chose) = (cx.entity().downgrade(), cx.entity().downgrade());
        let name = self.states[i].0;
        MergeButton::new(id, self.states[i].1.clone(), self.choice(i))
            .compact(compact)
            .on_action(move |action, _, cx| {
                acted.update(cx, |s, cx| s.report(format!("{} on \"{name}\"", action.word()), cx)).ok();
            })
            .on_choice(move |choice, _, cx| {
                chose
                    .update(cx, |s, cx| {
                        s.choices[i] = choice;
                        s.remember(choice.method, cx);
                    })
                    .ok();
            })
    }
}

impl Render for MergeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let heard = div()
            .text_size(TextSize::Xs.font_size())
            .text_color(muted)
            .child(match (&self.last, self.remembered.get(REPO)) {
                (Some(last), Some(method)) => format!("Pressed: {last}. Remembered for {REPO}: {}.", method.word()),
                (Some(last), None) => format!("Pressed: {last}."),
                (None, Some(method)) => format!("Remembered for {REPO}: {}.", method.word()),
                (None, None) => "Nothing pressed yet.".into(),
            });
        let rows = (0..self.states.len()).map(|i| {
            let (name, facts) = &self.states[i];
            let standing = beui::merge::standing(facts);
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .h(px(40.))
                .child(div().w(px(170.)).flex_none().text_size(TextSize::Sm.font_size()).child(*name))
                .child(div().flex_1().min_w_0().truncate().text_size(TextSize::Xs.font_size()).text_color(muted).child(standing))
                .child(self.button(i, ("merge-state", i), false, cx))
        });
        let cards = [(0, PrState::Open, Checks { passed: 3, ..Default::default() }, ReviewState::Approved), (13, PrState::Merged, Checks { passed: 3, ..Default::default() }, ReviewState::Approved)]
            .map(|(i, state, checks, review)| {
                let (acted, chose) = (cx.entity().downgrade(), cx.entity().downgrade());
                PrCard::new(("merge-card", i), PrChipData { state, ..pr_3344() })
                    .checks(checks)
                    .review(review)
                    .on_open(|pr, _, _| println!("open #{}", pr.number))
                    .merge(self.states[i].1.clone(), self.choice(i))
                    .on_merge(move |action, _, cx| {
                        acted.update(cx, |s, cx| s.report(format!("{} on the card", action.word()), cx)).ok();
                    })
                    .on_merge_choice(move |choice, _, cx| {
                        chose
                            .update(cx, |s, cx| {
                                s.choices[i] = choice;
                                s.remember(choice.method, cx);
                            })
                            .ok();
                    })
            });
        div()
            .child(div().pb(px(20.)).child(heard))
            .child(section("The button, in each state", narrow(div().flex().flex_col().children(rows))))
            .child(section("The PR card, open and merged", narrow(div().flex().flex_col().gap(px(8.)).children(cards))))
            .child(section(
                "The merge box: ready, blocked, merged",
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(16.))
                    .child(div().w(px(340.)).child(self.ready.clone()))
                    .child(div().w(px(340.)).child(self.blocked.clone()))
                    .child(div().w(px(340.)).child(self.merged.clone())),
            ))
    }
}
