use std::{collections::HashSet, sync::Arc};

use atelier_ui::{ActiveTheme, Button, ButtonSize, ButtonVariant};
use gpui_kit::{
    AppContext,
    Context,
    Entity,
    EventEmitter,
    FontWeight,
    IntoElement,
    ParentElement,
    Render,
    SharedString,
    Styled,
    Subscription,
    Window,
    div,
    px,
};
use atelier_forge::{Forge, PullRef, PullState};
use atelier_project::Project;

use crate::{
    list_view::{ListEvent, PullList},
    services::{PrConfig, Services},
    view::{PullEvent, PullView},
};
use super::types::PrEvent;

pub(super) struct Open {
    pub(super) reference: PullRef,
    pub(super) view: Entity<PullView>,
    pub(super) _events: Subscription,
}

pub struct PrHub {
    pub(super) services: Arc<Services>,
    pub(super) list: Entity<PullList>,
    pub(super) open: Option<Open>,
    /// A session the app linked to a pull request, by the words it shows on the button.
    pub(super) linked: Vec<(PullRef, SharedString)>,
    /// Pull requests whose checkout has been cleaned already.
    pub(super) swept: HashSet<PullRef>,
    pub(super) _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PrEvent> for PrHub {}

impl PrHub {
    /// Opens the reader's database and the caches (small local files) and starts reading the list.
    pub fn new(project: Arc<dyn Project>, forge: Arc<dyn Forge>, config: PrConfig, cx: &mut Context<Self>) -> Result<Self, String> {
        Ok(Self::with_services(Services::open(project, forge, config)?, cx))
    }

    /// The hub on services the app opened itself, off the UI thread (`Services::open` reads files).
    pub fn with_services(services: Arc<Services>, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| PullList::new(services.clone(), cx));
        let subscriptions = vec![cx.subscribe(&list, |_, _, _: &ListEvent, _| {}), cx.observe(&list, |hub, list, cx| hub.sweep(&list, cx))];
        Self { services, list, open: None, linked: Vec::new(), swept: HashSet::new(), _subscriptions: subscriptions }
    }

    /// The list, for an app that wants to open it in a pane of its own.
    pub fn list(&self) -> &Entity<PullList> {
        &self.list
    }

    /// The pull request on screen, if one is open.
    pub fn open_pull(&self) -> Option<&PullRef> {
        self.open.as_ref().map(|o| &o.reference)
    }

    /// The view of the pull request on screen, if one is open.
    pub fn view(&self) -> Option<&Entity<PullView>> {
        self.open.as_ref().map(|o| &o.view)
    }

    /// Shows a pull request. From a PR card, a PR chip, or a row of the list.
    pub fn open(&mut self, reference: PullRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.open.as_ref().is_some_and(|o| o.reference == reference) {
            return;
        }
        let services = self.services.clone();
        let view = cx.new(|cx| PullView::new(reference.clone(), services, window, cx));
        let events = cx.subscribe(&view, |hub, _, event: &PullEvent, cx| match event {
            PullEvent::OpenFile { pull, path, line } => cx.emit(PrEvent::OpenFile { pull: pull.clone(), path: path.clone(), line: *line }),
            PullEvent::StateChanged { pull, state } => {
                if matches!(state, PullState::Merged | PullState::Closed) {
                    hub.forget_checkout(pull.clone(), cx);
                }
            }
        });
        self.list.update(cx, |list, cx| list.opened(&reference, cx));
        gpui_kit::Focusable::focus_handle(view.read(cx), cx).focus(window, cx);
        self.open = Some(Open { reference, view, _events: events });
        cx.notify();
    }

    /// Back to the list. The pull request's checkout goes if it has closed.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if let Some(open) = self.open.take() {
            let closed = open.view.read(cx).model.closed();
            if closed {
                self.forget_checkout(open.reference.clone(), cx);
            }
            cx.emit(PrEvent::Closed(open.reference));
        }
        cx.notify();
    }

    /// The app has a session for this pull request (`None`: no longer). The view shows a button for it.
    pub fn set_linked_session(&mut self, reference: PullRef, label: Option<SharedString>, cx: &mut Context<Self>) {
        self.linked.retain(|(r, _)| *r != reference);
        if let Some(label) = label {
            self.linked.push((reference, label));
        }
        cx.notify();
    }

    /// Removes a closed pull request's head checkout and the refs it fetched, in the background.
    pub fn forget_checkout(&mut self, reference: PullRef, cx: &mut Context<Self>) {
        if !self.swept.insert(reference.clone()) || self.open_pull() == Some(&reference) {
            self.swept.remove(&reference);
            return;
        }
        let services = self.services.clone();
        cx.background_spawn(async move {
            let _ = services.git.remove(&reference);
            services.snapshots.remove(&reference);
        })
        .detach();
    }

    /// After the list is read: pull requests it shows as merged or closed lose their checkout.
    pub(super) fn sweep(&mut self, list: &Entity<PullList>, cx: &mut Context<Self>) {
        let closed = list.read(cx).model().closed();
        for reference in closed {
            if !self.swept.contains(&reference) {
                self.forget_checkout(reference, cx);
            }
        }
    }
}

impl Render for PrHub {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(open) = &self.open else {
            return div().size_full().bg(theme.background).child(self.list.clone()).into_any_element();
        };
        let linked = self.linked.iter().find(|(r, _)| *r == open.reference).map(|(_, label)| label.clone());
        let back = cx.listener(|hub, _, _, cx| hub.close(cx));
        let session = {
            let reference = open.reference.clone();
            cx.listener(move |_, _, _, cx| cx.emit(PrEvent::OpenSession(reference.clone())))
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .h(px(40.))
                    .px(px(10.))
                    .child(Button::new("pr-back").label("Pull requests").icon(atelier_ui::IconName::ArrowBack).variant(ButtonVariant::Ghost).size(ButtonSize::Sm).on_click(move |e, w, cx| back(e, w, cx)))
                    .child(div().font_weight(FontWeight::MEDIUM).text_color(theme.muted_foreground).child(format!("{} #{}", open.reference.repo.slug(), open.reference.number)))
                    .child(div().flex_1())
                    .children(linked.map(|label| Button::new("pr-session").label(label).variant(ButtonVariant::Secondary).size(ButtonSize::Sm).on_click(move |e, w, cx| session(e, w, cx)))),
            )
            .child(div().flex_1().min_h_0().child(open.view.clone()))
            .into_any_element()
    }
}
