use std::rc::Rc;

use atelier_plugin::{AppHost, Plugin, PluginView, Registry};
use gpui_kit::{
    AnyElement, App, ElementId, Entity, Global, SharedString, WeakEntity, Window,
};

use super::types::{Column, RenderCard, Visible};
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

/// What the modules of the app have added, by slot. A global: the shell and the bar read it each time they draw.
#[derive(Clone, Default)]
pub struct Slots {
    cards: Vec<StatusBarCard>,
    /// What the plugins added: their views.
    plugins: Registry,
}

impl Global for Slots {}

impl Slots {
    /// Adds a card; one already there with the same id is replaced.
    pub fn add_card(&mut self, card: StatusBarCard) {
        self.cards.retain(|have| have.id != card.id);
        self.cards.push(card);
    }

    /// Adds what `plugin` registers. This is the one line a plugin has in the app.
    pub fn plug(&mut self, plugin: &dyn Plugin) {
        self.plugins.add(plugin);
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

    /// The view a plugin registered as `id`.
    pub fn view(&self, id: &str) -> Option<&PluginView> {
        self.plugins.view(id)
    }

    /// Every view the plugins registered, in the order of the rail.
    pub fn views(&self) -> Vec<&PluginView> {
        self.plugins.views()
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

    /// Brings the view a plugin registered as `id` in front. Nothing happens for an id nobody registered.
    pub fn open_view(&self, id: &str, cx: &mut App) {
        drop(self.shell.update(cx, |shell, cx| shell.open_view(id, cx)));
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

/// The app's side of the handle a plugin gets (`atelier_plugin::Host`).
impl AppHost for Host {
    /// At the next turn of the app, not now: a page may ask while the shell draws it, and the shell cannot be changed then.
    fn open_view(&self, id: &str, cx: &mut App) {
        let (host, id) = (self.clone(), id.to_string());
        cx.defer(move |cx| Host::open_view(&host, &id, cx));
    }

    fn vitals(&self, cx: &App) -> atelier_plugin::Vitals {
        let vitals = self.vitals.read(cx);
        atelier_plugin::Vitals { load: vitals.load().cloned(), providers: vitals.providers().to_vec() }
    }

    /// Once the handler that asked has returned, so the shell can be changed.
    fn start_session_as(&self, bot: &str, window: &mut Window, cx: &mut App) {
        let (shell, bot) = (self.shell.clone(), bot.to_string());
        window.defer(cx, move |window, cx| drop(shell.update(cx, |shell, cx| shell.start_session_of_bot(&bot, window, cx))));
    }
}
