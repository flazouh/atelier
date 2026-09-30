//! The pull request form, in the Ship strip after a push: a title and a body the agent drafts and the
//! reader edits, the base branch, and whether it opens as a draft. Open (⌘↵) asks the forge off the UI
//! thread; a refusal keeps the form and says why.
use std::sync::Arc;
use beui::{
    ActiveTheme, Field, IconName, TextInput,
    button::{Button, ButtonVariant},
    checkbox::Checkbox,
    select::{Select, SelectOption},
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Styled, Task, Window, actions,
    component::input::{InputState, Textarea, TextareaState},
    div, prelude::FluentBuilder, px,
};
use lathe_agents::session::Backend;
use lathe_forge::{Forge, NewPull, PullRef, RepoRef};
use lathe_project::Project;
use crate::ship::{branch, commit::git, pull, push};

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
    /// The branch already has this open pull request; the form offers no create.
    Existing(PullRef),
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
    /// The branch already has this open pull request, with its title.
    Existing(PullRef, SharedString),
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
    /// Commits HEAD has that origin lacks: Open pushes them first.
    pub ahead: usize,
    /// The bases are being fetched; the list shown is the one from before.
    pub checking: bool,
    pub drafting: bool,
    /// Why the last Open did not open, in the forge's words.
    pub refused: Option<SharedString>,
    pub title: Entity<InputState>,
    pub body: Entity<TextareaState>,
    base_focus: FocusHandle,
    draft_focus: FocusHandle,
    open_focus: FocusHandle,
    work: Task<()>,
    checks: Task<()>,
}

/// What the form reads before it opens.
struct Read {
    head: Option<String>,
    repo: Option<RepoRef>,
    bases: Vec<String>,
    ahead: usize,
    /// The open pull request the branch has, as the forge says.
    existing: Result<Option<lathe_forge::PullBrief>, String>,
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
            ahead: 0,
            checking: false,
            drafting: false,
            refused: None,
            title,
            body,
            base_focus: cx.focus_handle(),
            draft_focus: cx.focus_handle(),
            open_focus: cx.focus_handle(),
            work: Task::ready(()),
            checks: Task::ready(()),
        }
    }

    /// Reads the branch, the repository and the bases off the UI thread, then drafts.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = FormStage::Reading;
        self.refused = None;
        let (project, forge) = (self.project.clone(), self.forge.clone());
        let reading = cx.background_spawn(async move {
            let head = branch::current(project.as_ref());
            // The address as written names the repository, whatever an insteadOf rule sends git to.
            let url = git(project.as_ref(), &["config", "--get", "remote.origin.url"], None, None).ok();
            let repo = url.as_deref().and_then(|u| RepoRef::from_remote(u.trim()));
            let bases = head.as_deref().map(|h| pull::bases(project.as_ref(), h)).unwrap_or_default();
            let ahead = pull::ahead(project.as_ref());
            let existing = match (&repo, &head) {
                (Some(repo), Some(head)) => forge.open_pull_for(repo, head).map_err(|e| e.to_string()),
                _ => Ok(None),
            };
            Read { head, repo, bases, ahead, existing }
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
        match read.existing {
            Ok(Some(brief)) => {
                self.stage = FormStage::Existing(brief.reference.clone(), brief.title.into());
                cx.emit(FormEvent::Existing(brief.reference));
                return cx.notify();
            }
            Err(words) => {
                self.stage = FormStage::Failed(words.into());
                return cx.notify();
            }
            Ok(None) => {}
        }
        if read.bases.is_empty() {
            self.stage = FormStage::Failed("origin has no other branch to open a pull request into".into());
            return cx.notify();
        }
        (self.head, self.repo, self.bases, self.base, self.ahead) = (head, Some(repo), read.bases, 0, read.ahead);
        self.stage = FormStage::Open;
        self.title.update(cx, |t, cx| t.focus(window, cx));
        self.check(window, cx);
        self.redraft(window, cx);
    }

    /// Fetches origin once, pruning gone branches, and reads the bases and what HEAD has ahead again.
    /// The list shown stays until it lands, and the base picked stays picked while it still exists.
    fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.checking = true;
        let (project, head) = (self.project.clone(), self.head.clone());
        let checking = cx.background_spawn(async move {
            let fetched = push::remote_git(project.as_ref(), &["fetch", "--prune", "--quiet", "origin"]);
            (fetched.is_ok(), pull::bases(project.as_ref(), &head), pull::ahead(project.as_ref()))
        });
        self.checks = cx.spawn_in(window, async move |this, cx| {
            let (fetched, bases, ahead) = checking.await;
            _ = this.update(cx, |form, cx| {
                form.checking = false;
                if fetched && !bases.is_empty() {
                    let picked = form.bases.get(form.base).cloned();
                    form.base = picked.and_then(|p| bases.iter().position(|b| *b == p)).unwrap_or(0);
                    (form.bases, form.ahead) = (bases, ahead);
                }
                cx.notify();
            });
        });
    }

    /// The Open button's words: it pushes first when HEAD has commits origin lacks.
    pub fn open_label(&self) -> &'static str {
        if self.ahead > 0 { "Push and open pull request" } else { "Open pull request" }
    }

    /// The form's Tab stops, in order: the title, the body, the base, the draft switch, and Open.
    #[cfg(test)]
    pub fn tab_stops(&self, cx: &gpui_kit::App) -> Vec<FocusHandle> {
        vec![self.title.focus_handle(cx), self.body.focus_handle(cx), self.base_focus.clone(), self.draft_focus.clone(), self.open_focus.clone()]
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
        let (forge, project, ahead) = (self.forge.clone(), self.project.clone(), self.ahead);
        let opening = cx.background_spawn(async move {
            // Commits made after the last push go first, or the pull request lacks them.
            if ahead > 0 {
                push::push(project.as_ref(), &new.head).map_err(|e| e.to_string())?;
            }
            forge.create_pull(&repo, &new).map_err(|e| e.to_string())
        });
        self.work = cx.spawn_in(window, async move |this, cx| {
            let result = opening.await;
            _ = this.update(cx, |form, cx| {
                match result {
                    Ok(reference) => {
                        form.stage = FormStage::Opened(reference.clone());
                        cx.emit(FormEvent::Opened(reference));
                    }
                    Err(words) => {
                        form.stage = FormStage::Open;
                        form.refused = Some(words.into());
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
            FormStage::Existing(reference, title) => {
                card.child(words(format!("This branch already has pull request #{}: {title}", reference.number).into())).into_any_element()
            }
            FormStage::Open | FormStage::Opening => {
                let opening = self.stage == FormStage::Opening;
                let body_focus = self.body.focus_handle(cx);
                let options: Vec<SelectOption> = self.bases.iter().map(|b| SelectOption::new(b.clone(), IconName::PrOpen)).collect();
                let (pick, drafts, cancel, open) = (this.clone(), this.clone(), this.clone(), this.clone());
                card.key_context(CONTEXT)
                    .on_action({
                        let this = this.clone();
                        move |_: &OpenNow, window, cx| drop(this.update(cx, |form, cx| form.submit(window, cx)))
                    })
                    .child(div().text_size(TextSize::Sm.font_size()).child(format!("Open a pull request from {}", self.head)))
                    .child(TextInput::new("pull-title", &self.title).surface(theme.card))
                    .child(Field::new(body_focus, Textarea::new(&self.body).appearance(false)).radius(radius::MD).padding(px(6.)))
                    .when(self.drafting, |d| d.child(words("The agent is drafting…".into())))
                    .when(self.checking, |d| d.child(words("Checking branches…".into())))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(words("Into".into()))
                            .child(Select::new("pull-base", options).selected(Some(self.base)).compact(true).focus_handle(&self.base_focus).on_change(move |at, _, cx| {
                                _ = pick.update(cx, |form, cx| form.set_base(at, cx));
                            }))
                            .child(Checkbox::new("pull-draft", self.draft).label("Open as a draft").focus_handle(&self.draft_focus).on_change(move |on, _, cx| {
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
                                    .label(if opening { "Opening…" } else { self.open_label() })
                                    .focus_handle(self.open_focus.clone())
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
