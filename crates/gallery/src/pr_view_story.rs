//! The "Pull request view" story: the real view (`lathe-pr-view`) on a real git repository and an
//! in-memory forge. The repository is made on disk when the story opens: a small crate "relay" on main, and
//! pull request 3344 with six commits, fetched into a cache and read by git the way a project's would be.
//! The forge holds its threads, remarks and checks (one failing job with its log), the reader's last
//! review, and a working set for the list. Writes go to the in-memory forge and nowhere else.
//!
//! `PRV_VIEW=list` opens the list first; the default opens the pull request. `PRV_LSP=1` starts the language
//! servers on the head's checkout (rust-analyzer, downloaded once unless `LATHE_OFFLINE` is set).
//! `PRV_BIG=1` opens a large pull request (`PRV_FILES`, `PRV_COMMENTS`); `GALLERY_SCROLL=1` measures frames on it.
use beui::ActiveTheme;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div};
use lathe_pr_view::{
    fixture::{big::Big, relay::Relay},
    hub::{PrEvent, PrHub},
};

use crate::sidebar_story::run::Run;

/// The repository the story reads, kept on disk while the story shows it.
#[allow(dead_code)]
enum Keep {
    _Relay(Relay),
    _Big(Big),
}

pub struct PrViewStory {
    hub: Entity<PrHub>,
    _keep: Keep,
    _events: Subscription,
    /// A measuring run (`GALLERY_SCROLL=1`): the rail scrolls and the walk goes through the files.
    run: Option<Run>,
}

impl PrViewStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let measuring = std::env::var("GALLERY_SCROLL").is_ok_and(|v| v == "1");
        let local = std::env::temp_dir().join("lathe-gallery-pr-view");
        let number = |name: &str, or: usize| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(or);
        let (hub, keep, reference) = if measuring || std::env::var("PRV_BIG").is_ok() {
            // PRV_FILES changed files and PRV_COMMENTS comments (five to a thread).
            let big = Big::build(number("PRV_FILES", 300), number("PRV_COMMENTS", 5000) / 5, 5);
            let config = big.config(&local).refresh(std::time::Duration::from_secs(600), std::time::Duration::from_secs(600));
            let hub = cx.new(|cx| PrHub::new(big.repo.project(), big.forge.clone(), config, cx).expect("the caches open"));
            let reference = big.reference.clone();
            (hub, Keep::_Big(big), reference)
        } else {
            let relay = Relay::build();
            let mut config = relay.config(&local).refresh(std::time::Duration::from_secs(5), std::time::Duration::from_secs(5));
            if std::env::var("PRV_LSP").is_ok_and(|v| v == "1") {
                config = config.workers(crate::workers::workers());
            }
            let hub = cx.new(|cx| PrHub::new(relay.repo.project(), relay.forge.clone(), config, cx).expect("the caches open"));
            let reference = relay.reference.clone();
            (hub, Keep::_Relay(relay), reference)
        };
        let events = cx.subscribe(&hub, |_, _, event: &PrEvent, _| println!("pull request view: {event:?}"));
        if std::env::var("PRV_VIEW").ok().as_deref() != Some("list") {
            hub.update(cx, |hub, cx| hub.open(reference, window, cx));
        }
        Self { hub, _keep: keep, _events: events, run: measuring.then(Run::new) }
    }
}
impl Render for PrViewStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().bg(cx.theme().background).child(self.hub.clone());
        let Some(run) = &mut self.run else { return root.into_any_element() };
        window.request_animation_frame();
        // The run starts when the first file is on screen: the opening has its own numbers.
        let view = self.hub.read(cx).view().cloned();
        let ready = view.as_ref().is_some_and(|v| v.read(cx).shows_a_file());
        if !ready {
            return root.into_any_element();
        }
        match run.frame() {
            Some(n) => {
                if let Some(view) = view {
                    let switched = view.update(cx, |v, cx| v.measure_step(n, window, cx));
                    if switched {
                        run.switched();
                    }
                }
                run.wrap(root.into_any_element()).into_any_element()
            }
            None => {
                if let Some(view) = &view {
                    for (step, at) in &view.read(cx).timeline {
                        println!("opening: {step:<24} {:>8.1} ms", at.as_secs_f64() * 1000.);
                    }
                }
                cx.quit();
                root.into_any_element()
            }
        }
    }
}
