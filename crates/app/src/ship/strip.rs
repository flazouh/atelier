//! The Ship strip, under the review bar: what the review kept, as one commit. Closed, it offers Commit
//! (⌘⇧C). Open, it lists what the commit takes against HEAD, file by file with +N −M (the text before
//! the turn can hold the reader's own uncommitted edits, and those go in too), asks for a new branch on
//! the default branch, and holds the message; Commit is ⌘↵. The agent drafts the branch name and the
//! message (`Backend::draft`); the reader edits both, and a draft never replaces what they typed. Every
//! git call and draft runs off the UI thread.

use std::sync::Arc;

use beui::{
    ActionSwapButton,
    ActiveTheme, Field,
    button::{Button, ButtonVariant},
    keys::Command as Key,
    theme::radius,
    typography::{MONO_FONT_FAMILY, TextSize},
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, Focusable, IntoElement, InteractiveElement, ParentElement, Render, SharedString, Styled, Task,
    Window, actions,
    component::input::{Input, InputState, Textarea, TextareaState},
    div, prelude::FluentBuilder, px,
};
use lathe_agents::session::Backend;
use lathe_forge::{Forge, PullRef};
use lathe_project::Project;

use crate::ship::{
    branch,
    commit::{self, CommitError},
    push,
    pull_form::{FormEvent, PullForm},
    drafts, head,
    kept::{Kept, against},
};

actions!(ship, [
    /// Commits what the strip holds.
    CommitNow,
]);

/// The context the strip's fields sit in, for ⌘↵.
pub const CONTEXT: &str = "ShipComposer";

pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([gpui_kit::KeyBinding::new("secondary-enter", CommitNow, Some("ShipComposer > Input"))]);
}

pub enum StripEvent {
    /// The reader asked to commit: the owner opens the strip with what the review kept.
    WantsOpen,
    /// A commit of these paths, with its full id.
    Committed { sha: String, paths: Vec<String> },
    /// A new branch is checked out, though the commit on it was refused.
    BranchMade,
    /// The branch went to origin.
    Pushed,
    /// A rebase gave the branch's own commits new ids: each old id with its new one.
    Rewrote(Vec<(String, String)>),
    /// The forge opened this pull request for the branch, or the branch had it already.
    PullOpened(PullRef),
    /// The reader asked to see this pull request.
    ShowPull(PullRef),
}

/// Why a pull and rebase stopped, for the strip.
enum Stop {
    OtherEdits,
    Words(String),
}

impl EventEmitter<StripEvent> for ShipStrip {}

/// Where the strip is.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Closed,
    /// Reading HEAD and the branch.
    Reading,
    Open,
    Committing,
    /// The last commit: its words for the strip.
    Committed(SharedString),
    Failed(SharedString),
    /// A push, or a pull and a rebase, running: its words.
    Pushing(SharedString),
    /// The branch is on origin: the words that say so.
    Pushed(SharedString),
    /// The remote has commits the branch lacks; Pull and rebase is offered.
    Rejected,
    /// Pull and rebase stopped before it started, for the reader's other edits; setting them aside is
    /// offered.
    EditsInTheWay,
    /// Pushed, but the edits set aside clash with the new commits: the words say where they are.
    Clashed(SharedString),
    /// The pull request form shows.
    Pull,
    /// The pull request is open: the words that say so.
    PullOpened(SharedString),
}

/// One file as the commit takes it: its path and its rows added and removed against HEAD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

pub struct ShipStrip {
    project: Arc<dyn Project>,
    backend: Arc<dyn Backend>,
    model: Option<String>,
    pub stage: Stage,
    kept: Vec<Kept>,
    pub lines: Vec<Line>,
    /// The branch checked out; `on_default` when it is the default branch, and a new one is asked for.
    pub branch: Option<String>,
    pub on_default: bool,
    pub drafting: bool,
    /// Why the last Commit did not commit, such as a hook's words; the card stays open.
    pub refused: Option<SharedString>,
    pub message: Entity<TextareaState>,
    pub new_branch: Entity<InputState>,
    /// The forge that opens the pull request.
    forge: Arc<dyn Forge>,
    /// The pull request form, while it shows.
    pub pull: Option<Entity<PullForm>>,
    /// The branch's pull request, once it has one: "Open #N" shows it.
    pub opened: Option<PullRef>,
    _pull: Option<gpui_kit::Subscription>,
    work: Task<()>,
}

impl ShipStrip {
    pub fn new(project: Arc<dyn Project>, backend: Arc<dyn Backend>, model: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 10).placeholder("What this commit does"));
        let new_branch = cx.new(|cx| InputState::new(window, cx).placeholder("fix/what-it-does"));
        let forge: Arc<dyn Forge> = Arc::new(lathe_forge::github::GitHub::new(project.clone()));
        Self {
            project,
            backend,
            model,
            stage: Stage::Closed,
            kept: Vec::new(),
            lines: Vec::new(),
            branch: None,
            on_default: false,
            drafting: false,
            refused: None,
            message,
            new_branch,
            forge,
            pull: None,
            opened: None,
            _pull: None,
            work: Task::ready(()),
        }
    }

    /// Opens the strip on `kept`: reads what HEAD has of each file and the branch, then asks the agent
    /// for the drafts.
    pub fn open(&mut self, kept: Vec<Kept>, window: &mut Window, cx: &mut Context<Self>) {
        if kept.is_empty() {
            self.stage = Stage::Failed(CommitError::Nothing.to_string().into());
            return cx.notify();
        }
        self.stage = Stage::Reading;
        self.refused = None;
        self.kept = kept.clone();
        let project = self.project.clone();
        let reading = cx.background_spawn(async move {
            let paths: Vec<&str> = kept.iter().map(|k| k.path.as_str()).collect();
            let heads = head::texts(project.as_ref(), &paths);
            let branch = branch::current(project.as_ref());
            let on_default = branch.as_deref().is_some_and(|b| branch::is_default(project.as_ref(), b));
            (heads, branch, on_default)
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let (heads, branch, on_default) = reading.await;
            _ = this.update_in(cx, |strip, window, cx| strip.opened(heads, branch, on_default, window, cx));
        });
        cx.notify();
    }

    fn opened(&mut self, heads: Vec<Option<String>>, branch: Option<String>, on_default: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.lines = self
            .kept
            .iter()
            .zip(&heads)
            .map(|(kept, head)| {
                let (added, removed) = against(head.as_deref(), kept);
                Line { path: kept.path.clone(), added, removed }
            })
            .collect();
        (self.branch, self.on_default) = (branch, on_default);
        self.stage = Stage::Open;
        self.drafting = true;
        self.message.update(cx, |m, cx| m.focus(window, cx));
        let diff = drafts::kept_diff(&heads.into_iter().zip(self.kept.iter().cloned()).collect::<Vec<_>>());
        let (project, backend, model) = (self.project.clone(), self.backend.clone(), self.model.clone());
        let drafting = cx.background_spawn(async move {
            let message = backend.draft(project.as_ref(), &drafts::commit_prompt(&diff), model.as_deref()).map(|d| drafts::message(&d));
            let branch = on_default.then(|| backend.draft(project.as_ref(), &drafts::branch_prompt(&diff), model.as_deref()).ok().and_then(|d| drafts::branch_name(&d)).map(|name| branch::free(project.as_ref(), &name)));
            (message, branch.flatten())
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let (message, branch) = drafting.await;
            _ = this.update_in(cx, |strip, window, cx| {
                strip.drafting = false;
                // A draft fills only a field the reader has not typed in.
                if let Ok(message) = message
                    && strip.message.read(cx).value().is_empty()
                {
                    strip.message.update(cx, |m, cx| m.set_value(message, window, cx));
                }
                if let Some(branch) = branch
                    && strip.new_branch.read(cx).value().is_empty()
                {
                    strip.new_branch.update(cx, |b, cx| b.set_value(branch, window, cx));
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    /// Pushes the branch checked out to origin, never forced. After a rejection, the push key pulls and
    /// rebases first.
    pub fn push(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.stage {
            Stage::Rejected => return self.pull_and_rebase(window, cx),
            Stage::EditsInTheWay => return self.set_aside_and_rebase(window, cx),
            Stage::Reading | Stage::Open | Stage::Committing | Stage::Pushing(_) | Stage::Pull => return,
            Stage::Closed | Stage::Committed(_) | Stage::Failed(_) | Stage::Pushed(_) | Stage::Clashed(_) | Stage::PullOpened(_) => {}
        }
        self.stage = Stage::Pushing("Pushing…".into());
        let project = self.project.clone();
        let pushing = cx.background_spawn(async move {
            let branch = branch::current(project.as_ref()).ok_or(None)?;
            push::push(project.as_ref(), &branch).map(|()| branch).map_err(Some)
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let result = pushing.await;
            _ = this.update(cx, |strip, cx| {
                strip.stage = match result {
                    Ok(branch) => {
                        cx.emit(StripEvent::Pushed);
                        Stage::Pushed(format!("Pushed {branch} to origin").into())
                    }
                    Err(Some(push::PushError::Rejected)) => Stage::Rejected,
                    Err(Some(error)) => Stage::Failed(error.to_string().into()),
                    Err(None) => Stage::Failed("HEAD is not on a branch: check one out to push".into()),
                };
                cx.notify();
            });
        });
        cx.notify();
    }

    /// After a rejection: pulls origin's branch, puts the commits on top, and pushes. A conflict stops
    /// it with nothing changed; the reader's other edits stop it before it starts, and the strip offers
    /// to set them aside.
    pub fn pull_and_rebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage == Stage::Rejected {
            self.rebase_and_push(false, window, cx);
        }
    }

    /// On the reader's word, after other edits stopped Pull and rebase: sets them aside, rebases,
    /// pushes, and puts them back.
    pub fn set_aside_and_rebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage == Stage::EditsInTheWay {
            self.rebase_and_push(true, window, cx);
        }
    }

    fn rebase_and_push(&mut self, set_aside: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = Stage::Pushing(if set_aside { "Setting your edits aside, rebasing and pushing…" } else { "Pulling, rebasing and pushing…" }.into());
        let project = self.project.clone();
        let running = cx.background_spawn(async move {
            let branch = branch::current(project.as_ref()).ok_or(Stop::Words("HEAD is not on a branch".into()))?;
            let rebased = if set_aside {
                push::pull_rebase_setting_aside(project.as_ref(), &branch)
            } else {
                push::pull_rebase(project.as_ref(), &branch)
            };
            let rebased = match rebased {
                Ok(rebased) => rebased,
                Err(push::RebaseError::OtherEdits) => return Err(Stop::OtherEdits),
                Err(error) => return Err(Stop::Words(error.to_string())),
            };
            // The rebase stands whatever the push says, so the owner hears of the new ids either way.
            let pushed = push::push(project.as_ref(), &branch).map_err(|e| e.to_string());
            Ok((branch, rebased, pushed))
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let result = running.await;
            _ = this.update(cx, |strip, cx| {
                strip.stage = match result {
                    Ok((branch, rebased, pushed)) => {
                        if !rebased.moved.is_empty() {
                            cx.emit(StripEvent::Rewrote(rebased.moved.clone()));
                        }
                        match (pushed, rebased.edits) {
                            (Err(words), _) => Stage::Failed(words.into()),
                            (Ok(()), edits) => {
                                cx.emit(StripEvent::Pushed);
                                let done = format!("Pulled, rebased and pushed {branch}");
                                match edits {
                                    None => Stage::Pushed(done.into()),
                                    Some(back @ push::PutBack::Back) => Stage::Pushed(format!("{done}. {back}").into()),
                                    Some(kept) => Stage::Clashed(format!("{done}. {kept}").into()),
                                }
                            }
                        }
                    }
                    Err(Stop::OtherEdits) => Stage::EditsInTheWay,
                    Err(Stop::Words(words)) => Stage::Failed(words.into()),
                };
                cx.notify();
            });
        });
        cx.notify();
    }

    /// The forge the pull request goes to, in place of GitHub through gh.
    #[cfg(test)]
    pub fn set_forge(&mut self, forge: Arc<dyn Forge>) {
        self.forge = forge;
    }

    /// Shows the pull request form for the branch checked out (⌘⇧R). Once the branch has its pull
    /// request, the same key shows that one.
    pub fn open_pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Stage::PullOpened(_), Some(reference)) = (&self.stage, &self.opened) {
            return cx.emit(StripEvent::ShowPull(reference.clone()));
        }
        if matches!(self.stage, Stage::Reading | Stage::Open | Stage::Committing | Stage::Pushing(_) | Stage::Pull) {
            return;
        }
        let (project, backend, model, forge) = (self.project.clone(), self.backend.clone(), self.model.clone(), self.forge.clone());
        let form = cx.new(|cx| PullForm::new(project, backend, model, forge, window, cx));
        self._pull = Some(cx.subscribe(&form, |strip, _, event: &FormEvent, cx| {
            match event {
                FormEvent::Opened(reference) => {
                    strip.stage = Stage::PullOpened(format!("Opened pull request #{}", reference.number).into());
                    strip.pull = None;
                    strip.opened = Some(reference.clone());
                    cx.emit(StripEvent::PullOpened(reference.clone()));
                }
                FormEvent::Existing(reference) => {
                    strip.stage = Stage::PullOpened(format!("This branch already has pull request #{}", reference.number).into());
                    strip.pull = None;
                    strip.opened = Some(reference.clone());
                    cx.emit(StripEvent::PullOpened(reference.clone()));
                }
                FormEvent::Cancelled => {
                    strip.stage = Stage::Closed;
                    strip.pull = None;
                }
            }
            cx.notify();
        }));
        form.update(cx, |f, cx| f.open(window, cx));
        self.pull = Some(form);
        self.stage = Stage::Pull;
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.stage = Stage::Closed;
        self.work = Task::ready(());
        cx.notify();
    }

    /// Commits the kept files with the message, on a new branch first when on the default one.
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stage != Stage::Open {
            return;
        }
        let message = self.message.read(cx).value().trim().to_string();
        if message.is_empty() {
            return;
        }
        let new_branch = self.on_default.then(|| self.new_branch.read(cx).value().trim().to_string());
        if new_branch.as_ref().is_some_and(String::is_empty) {
            self.refused = Some("Name the new branch: this is the default branch".into());
            return cx.notify();
        }
        self.stage = Stage::Committing;
        self.refused = None;
        let (project, kept) = (self.project.clone(), self.kept.clone());
        let committing = cx.background_spawn(async move {
            // The branch made here stays made when the commit is refused after it.
            if let Some(name) = &new_branch {
                branch::create(project.as_ref(), name).map_err(|words| (None, words))?;
            }
            let done = commit::commit(project.as_ref(), &kept, &message).map_err(|e| (new_branch.clone(), e.to_string()))?;
            Ok::<_, (Option<String>, String)>((done, branch::current(project.as_ref())))
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let result = committing.await;
            _ = this.update_in(cx, |strip, window, cx| {
                match result {
                    Ok((done, branch)) => {
                        let short: String = done.sha.chars().take(7).collect();
                        let on = branch.map(|b| format!(" on {b}")).unwrap_or_default();
                        strip.stage = Stage::Committed(format!("Committed {} as {short}{on}", files(strip.kept.len())).into());
                        // The next commit drafts its own words.
                        strip.message.update(cx, |m, cx| m.set_value("", window, cx));
                        strip.new_branch.update(cx, |b, cx| b.set_value("", window, cx));
                        let paths = strip.kept.iter().map(|k| k.path.clone()).collect();
                        cx.emit(StripEvent::Committed { sha: done.sha.clone(), paths });
                    }
                    Err((made, words)) => {
                        strip.stage = Stage::Open;
                        let words = match made {
                            Some(name) => {
                                (strip.branch, strip.on_default) = (Some(name.clone()), false);
                                cx.emit(StripEvent::BranchMade);
                                format!("{}. You are on {name} now; Commit tries again there.", words.trim_end_matches('.'))
                            }
                            None => words,
                        };
                        strip.refused = Some(words.into());
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}

impl Render for ShipStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (branch_focus, message_focus) = (self.new_branch.focus_handle(cx), self.message.focus_handle(cx));
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let words = |text: SharedString| div().text_size(TextSize::Xs.font_size()).text_color(muted).child(text);
        let this = cx.entity().downgrade();
        let commit_button = |label: &'static str| {
            let this = this.clone();
            ActionSwapButton::new("ship-primary", label).command(Key::Commit).on_click(move |_, window, cx| {
                _ = this.update(cx, |strip, cx| strip.commit(window, cx));
            })
        };
        let row = div().flex().items_center().gap(px(8.)).px(px(12.)).min_h(px(36.));
        match &self.stage {
            Stage::Pushing(w) => row.child(words(w.clone())).into_any_element(),
            Stage::Pull => match self.pull.clone() {
                Some(form) => form.into_any_element(),
                None => row.into_any_element(),
            },
            Stage::Rejected => {
                let (again, cancel) = (this.clone(), this.clone());
                row.child(div().flex_1().min_w_0().text_size(TextSize::Xs.font_size()).text_color(muted).child(push::PushError::Rejected.to_string()))
                    .child(Button::new("ship-cancel-push").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                        _ = cancel.update(cx, |strip, cx| strip.cancel(cx));
                    }))
                    .child(Button::new("ship-rebase").label("Pull and rebase").variant(ButtonVariant::Secondary).command(Key::Push).on_click(
                        move |_, window, cx| drop(again.update(cx, |strip, cx| strip.pull_and_rebase(window, cx))),
                    ))
                    .into_any_element()
            }
            Stage::EditsInTheWay => {
                let (aside, cancel) = (this.clone(), this.clone());
                row.child(div().flex_1().min_w_0().text_size(TextSize::Xs.font_size()).text_color(muted).child(push::RebaseError::OtherEdits.to_string()))
                    .child(Button::new("ship-cancel-aside").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                        _ = cancel.update(cx, |strip, cx| strip.cancel(cx));
                    }))
                    .child(
                        Button::new("ship-set-aside")
                            .label("Set my edits aside, rebase, and put them back")
                            .variant(ButtonVariant::Secondary)
                            .command(Key::Push)
                            .on_click(move |_, window, cx| drop(aside.update(cx, |strip, cx| strip.set_aside_and_rebase(window, cx)))),
                    )
                    .into_any_element()
            }
            Stage::Closed | Stage::Committed(_) | Stage::Failed(_) | Stage::Pushed(_) | Stage::Clashed(_) | Stage::PullOpened(_) => {
                let said = match &self.stage {
                    Stage::Committed(w) | Stage::Failed(w) | Stage::Pushed(w) | Stage::Clashed(w) | Stage::PullOpened(w) => Some(w.clone()),
                    _ => None,
                };
                let failed = matches!(self.stage, Stage::Failed(_) | Stage::Clashed(_));
                let pushable = matches!(self.stage, Stage::Committed(_) | Stage::Failed(_));
                let asks = this.clone();
                let pushes = this.clone();
                row.child(div().flex_1().min_w_0().children(said.map(|w| {
                    div().text_size(TextSize::Xs.font_size()).text_color(if failed { theme.danger } else { muted }).child(w)
                })))
                .when_some(self.opened.clone().filter(|_| matches!(self.stage, Stage::PullOpened(_))), |row, reference| {
                    let shows = this.clone();
                    row.child(ActionSwapButton::new("ship-primary", format!("Open #{}", reference.number)).command(Key::OpenPull).on_click(
                        move |_, window, cx| drop(shows.update(cx, |strip, cx| strip.open_pull(window, cx))),
                    ))
                })
                .when(matches!(self.stage, Stage::Pushed(_) | Stage::Clashed(_)), |row| {
                    let opens = this.clone();
                    row.child(ActionSwapButton::new("ship-primary", "Open pull request").command(Key::OpenPull).on_click(
                        move |_, window, cx| drop(opens.update(cx, |strip, cx| strip.open_pull(window, cx))),
                    ))
                })
                .when(pushable, |row| {
                    row.child(ActionSwapButton::new("ship-primary", "Push").command(Key::Push).on_click(
                        move |_, window, cx| drop(pushes.update(cx, |strip, cx| strip.push(window, cx))),
                    ))
                })
                .child(Button::new("ship-open").label("Commit what you kept").variant(ButtonVariant::Ghost).command(Key::Commit).on_click(
                    move |_, _, cx| drop(asks.update(cx, |_, cx| cx.emit(StripEvent::WantsOpen))),
                ))
                .into_any_element()
            }
            Stage::Reading => row.child(words("Reading what the commit takes…".into())).into_any_element(),
            Stage::Open | Stage::Committing => {
                let committing = self.stage == Stage::Committing;
                let lines = self.lines.iter().map(|line| {
                    div()
                        .flex()
                        .gap(px(8.))
                        .text_size(TextSize::Xs.font_size())
                        .child(div().flex_1().min_w_0().truncate().child(line.path.clone()))
                        .child(div().font_family(MONO_FONT_FAMILY).text_color(theme.diff_color(true)).child(format!("+{}", line.added)))
                        .child(div().font_family(MONO_FONT_FAMILY).text_color(theme.diff_color(false)).child(format!("\u{2212}{}", line.removed)))
                });
                let on = self.branch.clone().unwrap_or_else(|| "a detached HEAD".into());
                let cancel = this.clone();
                div()
                    .key_context(CONTEXT)
                    .on_action({
                        let this = this.clone();
                        move |_: &CommitNow, window, cx| drop(this.update(cx, |strip, cx| strip.commit(window, cx)))
                    })
                    .mx(px(6.))
                    .p(px(12.))
                    .rounded(radius::LG)
                    .bg(theme.card)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(TextSize::Sm.font_size()).child(format!("Commit what you kept, on {on}")))
                    .child(words("Against HEAD: the text before the turn may hold your own uncommitted edits.".into()))
                    .child(div().flex().flex_col().gap(px(2.)).children(lines))
                    .when(self.on_default, |d| {
                        d.child(words(format!("{on} is the default branch: this commit goes on a new one").into()))
                            .child(Field::new(branch_focus.clone(), Input::new(&self.new_branch).appearance(false)).radius(radius::MD))
                    })
                    .child(Field::new(message_focus.clone(), Textarea::new(&self.message).appearance(false)).radius(radius::MD).padding(px(6.)))
                    .when(self.drafting, |d| d.child(words("The agent is drafting…".into())))
                    .when_some(self.refused.clone(), |d, refused| {
                        d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(refused))
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(Button::new("ship-cancel").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                                _ = cancel.update(cx, |strip, cx| strip.cancel(cx));
                            }))
                            .child(commit_button(if committing { "Committing…" } else { "Commit" }).disabled(committing).cap(beui::keys::cap("⌘↵"))),
                    )
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests;
/// "1 file", or "N files".
fn files(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}
