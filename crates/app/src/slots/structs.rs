use std::rc::Rc;

use atelier_ui::IconName;
use gpui_kit::{
    AnyElement, AnyView, App, ElementId, Entity, Global, SharedString, WeakEntity, Window,
};

use super::types::{Column, OpenView, RenderCard, Visible};
use crate::{shell::Shell, vitals::Vitals};

/// What a module puts in one column of the status bar. The bar has one panel under each column; a card is a piece of one.
#[derive(Clone)]
pub struct StatusBarCard {
    /// Names the card. A card added with the id of another takes its place.
    pub id: SharedString,
    pub column: Column,
    /// Cards of a column stand from the lowest order to the highest; the same order goes by id.
    pub order: i32,
    pub(super) visible: Visible,
    pub render: RenderCard,
}

impl StatusBarCard {
    /// A card that is always drawn; [`when`](Self::when) limits it.
    pub fn new(
        id: impl Into<SharedString>,
        column: Column,
        order: i32,
        render: impl Fn(&BarEnv<'_>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            column,
            order,
            visible: Rc::new(|_| true),
            render: Rc::new(render),
        }
    }

    /// The card is drawn only while `visible` says so.
    pub fn when(mut self, visible: impl Fn(&Vitals) -> bool + 'static) -> Self {
        self.visible = Rc::new(visible);
        self
    }
}

/// A view a module opens, and the door to it. The door is an icon and a label on the left rail; a view with no icon has its
/// door elsewhere (the usage view opens from the chips in the status bar). The rail itself still draws its three built-in views.
#[derive(Clone)]
#[expect(
    dead_code,
    reason = "the rail still draws its three built-in views and not these"
)]
pub struct RailView {
    pub id: SharedString,
    pub icon: Option<IconName>,
    pub label: SharedString,
    /// Doors stand from the lowest order to the highest; the same order goes by id.
    pub order: i32,
    pub open: OpenView,
}

impl RailView {
    pub fn new(
        id: impl Into<SharedString>,
        icon: Option<IconName>,
        label: impl Into<SharedString>,
        order: i32,
        open: impl Fn(&Host, &mut App) -> AnyView + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            label: label.into(),
            order,
            open: Rc::new(open),
        }
    }
}

/// What the modules of the app have added, by slot. A global: the shell and the bar read it each time they draw.
#[derive(Clone, Default)]
pub struct Slots {
    cards: Vec<StatusBarCard>,
    views: Vec<RailView>,
}

impl Global for Slots {}

impl Slots {
    /// Adds a card; one already there with the same id is replaced.
    pub fn add_card(&mut self, card: StatusBarCard) {
        self.cards.retain(|have| have.id != card.id);
        self.cards.push(card);
    }

    /// Adds a view; one already there with the same id is replaced.
    pub fn add_view(&mut self, view: RailView) {
        self.views.retain(|have| have.id != view.id);
        self.views.push(view);
    }

    /// The cards to draw in `column` now, in order.
    pub fn cards(&self, column: Column, vitals: &Vitals) -> Vec<&StatusBarCard> {
        let mut cards: Vec<&StatusBarCard> = self
            .cards
            .iter()
            .filter(|card| card.column == column && (card.visible)(vitals))
            .collect();
        cards.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        cards
    }

    pub fn view(&self, id: &str) -> Option<&RailView> {
        self.views.iter().find(|view| view.id.as_ref() == id)
    }

    /// Every view, in order.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "for the rail, which still draws its three built-in views"
        )
    )]
    pub fn views(&self) -> Vec<&RailView> {
        let mut views: Vec<&RailView> = self.views.iter().collect();
        views.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        views
    }
}

/// What a card or a view may use of the app that shows it, and the few things it may ask the app to do.
#[derive(Clone)]
pub struct Host {
    shell: WeakEntity<Shell>,
    /// The numbers of the status bar.
    pub vitals: Entity<Vitals>,
}

impl Host {
    pub fn new(shell: WeakEntity<Shell>, vitals: Entity<Vitals>) -> Self {
        Self { shell, vitals }
    }

    /// Opens the view of the registered [`RailView`] named `id`, over the window. Nothing happens for an id nobody registered.
    pub fn open_view(&self, id: &str, cx: &mut App) {
        drop(self.shell.update(cx, |shell, cx| shell.open_view(id, cx)));
    }

    /// Closes the view that is open.
    pub fn close_view(&self, cx: &mut App) {
        drop(self.shell.update(cx, |shell, cx| shell.close_view(cx)));
    }

    /// Opens the changelog.
    pub fn show_changelog(&self, cx: &mut App) {
        drop(self.shell.update(cx, |shell, cx| shell.show_changelog(cx)));
    }
}

/// What a card draws with: the bar's id (its parts take their ids from it), the numbers, and the host.
pub struct BarEnv<'a> {
    pub id: &'a ElementId,
    pub vitals: &'a Vitals,
    pub host: &'a Host,
}
