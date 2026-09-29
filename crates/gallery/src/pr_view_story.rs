//! The "Pull request view" story: the real view (`lathe-pr-view`) on a real git repository and an
//! in-memory forge. The repository is made on disk when the story opens: a small crate "relay" on main, and
//! pull request 3344 with six commits, fetched into a cache and read by git the way a project's would be.
//! The forge holds its threads, remarks and checks (one failing job with its log), the reader's last
//! review, and a working set for the list. Writes go to the in-memory forge and nowhere else.
//!
//! `PRV_VIEW=list` opens the list first; the default opens the pull request. `PRV_LSP=1` starts the language
//! servers on the head's checkout (rust-analyzer, downloaded once unless `LATHE_OFFLINE` is set).
use beui::ActiveTheme;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div};
use lathe_pr_view::{
    fixture::relay::Relay,
    hub::{PrEvent, PrHub},
};

pub struct PrViewStory {
    hub: Entity<PrHub>,
    /// Keeps the repository on disk while the story shows it.
    _relay: Relay,
    _events: Subscription,
}

impl PrViewStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let relay = Relay::build();
        let local = std::env::temp_dir().join("lathe-gallery-pr-view");
        let mut config = relay.config(&local).refresh(std::time::Duration::from_secs(5), std::time::Duration::from_secs(10));
        if std::env::var("PRV_LSP").is_ok_and(|v| v == "1") {
            config = config.workers(crate::workers::workers());
        }
        let (project, forge, reference) = (relay.repo.project(), relay.forge.clone(), relay.reference.clone());
        let hub = cx.new(|cx| PrHub::new(project, forge, config, cx).expect("the caches open"));
        let events = cx.subscribe(&hub, |_, _, event: &PrEvent, _| println!("pull request view: {event:?}"));
        if std::env::var("PRV_VIEW").ok().as_deref() != Some("list") {
            hub.update(cx, |hub, cx| hub.open(reference, window, cx));
        }
        Self { hub, _relay: relay, _events: events }
    }
}

impl Render for PrViewStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.theme().background).child(self.hub.clone())
    }
}
