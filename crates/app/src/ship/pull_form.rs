//! The pull request form, in the Ship strip after a push: a title and a body the agent drafts and the
//! reader edits, the base branch, and whether it opens as a draft. Open (⌘↵) asks the forge off the UI
//! thread; a refusal keeps the form and says why.
use std::sync::Arc;
use beui::{
    ActiveTheme, Field, IconName,
    button::{Button, ButtonVariant},
    checkbox::Checkbox,
    select::{Select, SelectOption},
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Styled, Task, Window, actions,
    component::input::{Input, InputState, Textarea, TextareaState},
    div, prelude::FluentBuilder, px,
};
use lathe_agents::session::Backend;
use lathe_forge::{Forge, NewPull, PullRef, RepoRef};
use lathe_project::Project;
use crate::ship::{branch, commit::git, pull};

actions!(pull_form, [
    /// Opens the pull request the form holds.
    OpenNow,
]);
/// The context the form's fields sit in, for ⌘↵.
pub const CONTEXT: &str = "PullForm";
pub fn bind_keys(cx: &mut gpui_kit::App) {
    cx.bind_keys([gpui_kit::KeyBinding::new("secondary-enter", OpenNow, Some("PullForm > Input"))]);
}

pub enum FormEvent {
    Opened(PullRef),
    Cancelled,
}
impl EventEmitter<FormEvent> for PullForm {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormStage {
    /// Reading the branch, the repository and the bases.
    Reading,
    Open,
    Opening,
    Opened(PullRef),
    /// The form cannot open a pull request here: the words say why.
    Failed(SharedString),
}

pub struct PullForm {
    project: Arc<dyn Project>,
    backend: Arc<dyn Backend>,
    model: Option<String>,
    forge: Arc<dyn Forge>,
    pub stage: FormStage,
    pub head: String,
    repo: Option<RepoRef>,
    pub bases: Vec<String>,
    pub base: usize,
    pub draft: bool,
    pub drafting: bool,
    /// Why the last Open did not open, in the forge's words.
    pub refused: Option<SharedString>,
    pub title: Entity<InputState>,
    pub body: Entity<TextareaState>,
    work: Task<()>,
}

/// What the form reads before it opens.
struct Read {
    head: Option<String>,
    repo: Option<RepoRef>,
    bases: Vec<String>,
}

impl PullForm {
    pub fn new(project: Arc<dyn Project>, backend: Arc<dyn Backend>, model: Option<String>, forge: Arc<dyn Forge>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let body = cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 14).placeholder("What changes, and why"));
        Self {
            project,
            backend,
            model,
            forge,
            stage: FormStage::Reading,
            head: String::new(),
            repo: None,
            bases: Vec::new(),
            base: 0,
            draft: false,
            drafting: false,
            refused: None,
            title,
            body,
            work: Task::ready(()),
        }
    }

    /// Reads the branch, the repository and the bases off the UI thread, then drafts.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = FormStage::Reading;
        self.refused = None;
        let project = self.project.clone();
        let reading = cx.background_spawn(async move {
            let head = branch::current(project.as_ref());
            let url = git(project.as_ref(), &["remote", "get-url", "origin"], None, None).ok();
            let repo = url.as_deref().and_then(|u| RepoRef::from_remote(u.trim()));
            let bases = head.as_deref().map(|h| pull::bases(project.as_ref(), h)).unwrap_or_default();
            Read { head, repo, bases }
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let read = reading.await;
            _ = this.update_in(cx, |form, window, cx| form.read(read, window, cx));
        });
        cx.notify();
    }

    fn read(&mut self, read: Read, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(head), Some(repo)) = (read.head, read.repo) else {
            self.stage = FormStage::Failed(crate::open_project::NO_FORGE_REMOTE.into());
            return cx.notify();
        };
        if read.bases.is_empty() {
            self.stage = FormStage::Failed("origin has no other branch to open a pull request into".into());
            return cx.notify();
        }
        (self.head, self.repo, self.bases, self.base) = (head, Some(repo), read.bases, 0);
        self.stage = FormStage::Open;
        self.title.update(cx, |t, cx| t.focus(window, cx));
        self.redraft(window, cx);
    }

    /// Asks the agent for a title and a body against the base picked; a draft fills only an empty field.
    fn redraft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drafting = true;
        let (project, backend, model, base) = (self.project.clone(), self.backend.clone(), self.model.clone(), self.bases[self.base].clone());
        let drafting = cx.background_spawn(async move {
            let prompt = pull::prompt(project.as_ref(), &base);
            backend.draft(project.as_ref(), &prompt, model.as_deref()).map(|d| pull::title_and_body(&d))
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let drafted = drafting.await;
            _ = this.update_in(cx, |form, window, cx| {
                form.drafting = false;
                if let Ok((title, body)) = drafted {
                    if form.title.read(cx).value().is_empty() {
                        form.title.update(cx, |t, cx| t.set_value(title, window, cx));
                    }
                    if form.body.read(cx).value().is_empty() {
                        form.body.update(cx, |b, cx| b.set_value(body, window, cx));
                    }
                }
                cx.notify();
            });
        });
    }

    pub fn set_draft(&mut self, draft: bool, cx: &mut Context<Self>) {
        self.draft = draft;
        cx.notify();
    }

    pub fn set_base(&mut self, base: usize, cx: &mut Context<Self>) {
        if base < self.bases.len() {
            self.base = base;
            cx.notify();
        }
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.work = Task::ready(());
        cx.emit(FormEvent::Cancelled);
    }

    /// Asks the forge for the pull request, off the UI thread.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (FormStage::Open, Some(repo)) = (&self.stage, self.repo.clone()) else { return };
        let title = self.title.read(cx).value().trim().to_string();
        if title.is_empty() {
            self.refused = Some("A pull request needs a title".into());
            return cx.notify();
        }
        let new = NewPull {
            title,
            body: self.body.read(cx).value().trim().to_string(),
            base: self.bases[self.base].clone(),
            head: self.head.clone(),
            draft: self.draft,
        };
        self.stage = FormStage::Opening;
        self.refused = None;
        let forge = self.forge.clone();
        let opening = cx.background_spawn(async move { forge.create_pull(&repo, &new) });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let result = opening.await;
            _ = this.update(cx, |form, cx| {
                match result {
                    Ok(reference) => {
                        form.stage = FormStage::Opened(reference.clone());
                        cx.emit(FormEvent::Opened(reference));
                    }
                    Err(error) => {
                        form.stage = FormStage::Open;
                        form.refused = Some(error.to_string().into());
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}

impl Render for PullForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let words = |text: SharedString| div().text_size(TextSize::Xs.font_size()).text_color(muted).child(text);
        let this = cx.entity().downgrade();
        let card = div().mx(px(6.)).p(px(12.)).rounded(radius::LG).bg(theme.card).flex().flex_col().gap(px(8.));
        match &self.stage {
            FormStage::Reading => card.child(words("Reading the branch and its bases…".into())).into_any_element(),
            FormStage::Failed(why) => card.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(why.clone())).into_any_element(),
            FormStage::Opened(reference) => card.child(words(format!("Opened #{}", reference.number).into())).into_any_element(),
            FormStage::Open | FormStage::Opening => {
                let opening = self.stage == FormStage::Opening;
                let (title_focus, body_focus) = (self.title.focus_handle(cx), self.body.focus_handle(cx));
                let options: Vec<SelectOption> = self.bases.iter().map(|b| SelectOption::new(b.clone(), IconName::PrOpen)).collect();
                let (pick, drafts, cancel, open) = (this.clone(), this.clone(), this.clone(), this.clone());
                card.key_context(CONTEXT)
                    .on_action({
                        let this = this.clone();
                        move |_: &OpenNow, window, cx| drop(this.update(cx, |form, cx| form.submit(window, cx)))
                    })
                    .child(div().text_size(TextSize::Sm.font_size()).child(format!("Open a pull request from {}", self.head)))
                    .child(Field::new(title_focus, Input::new(&self.title).appearance(false)).radius(radius::MD))
                    .child(Field::new(body_focus, Textarea::new(&self.body).appearance(false)).radius(radius::MD).padding(px(6.)))
                    .when(self.drafting, |d| d.child(words("The agent is drafting…".into())))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(words("Into".into()))
                            .child(Select::new("pull-base", options).selected(Some(self.base)).compact(true).on_change(move |at, _, cx| {
                                _ = pick.update(cx, |form, cx| form.set_base(at, cx));
                            }))
                            .child(Checkbox::new("pull-draft", self.draft).label("Open as a draft").on_change(move |on, _, cx| {
                                _ = drafts.update(cx, |form, cx| form.set_draft(on, cx));
                            })),
                    )
                    .when_some(self.refused.clone(), |d, refused| d.child(div().text_size(TextSize::Xs.font_size()).text_color(theme.danger).child(refused)))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(Button::new("pull-cancel").label("Cancel").variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                                _ = cancel.update(cx, |form, cx| form.cancel(cx));
                            }))
                            .child(
                                Button::new("pull-open")
                                    .label(if opening { "Opening…" } else { "Open pull request" })
                                    .variant(ButtonVariant::Primary)
                                    .cap(beui::keys::cap("⌘↵"))
                                    .disabled(opening)
                                    .on_click(move |_, window, cx| drop(open.update(cx, |form, cx| form.submit(window, cx)))),
                            ),
                    )
                    .into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests;
