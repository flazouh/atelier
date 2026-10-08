//! The bar at the foot of the window: how hard this machine works, what the agents are doing, and how much of each
//! provider's allowance is used. The numbers live in [`Vitals`], an entity of its own, so a sample each second draws
//! the bar and not the whole window; the shell feeds it.

use std::{sync::Arc, time::SystemTime};

use atelier_project::Project;
use atelier_settings::secrets::{OPENROUTER_KEY, Secrets};
use atelier_ui::{Work, session_status::SessionStatus};
use gpui_kit::{AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Task, div};

use super::structs::Shell;
use crate::{
    providers::{self, Choice},
    vitals::{LOAD_EVERY, PROVIDERS_EVERY, SysinfoProbe, Vitals},
};

/// How long after launch the providers are first asked: after the window has its first frames.
const FIRST_ASK: std::time::Duration = std::time::Duration::from_secs(2);

/// What one asking of the providers needs: the host to ask on, whether the OpenRouter key is wanted, and where it is kept.
type Job = (Arc<dyn Project>, bool, Arc<dyn Secrets>);

impl Shell {
    /// The sessions at work and the sessions that wait on the reader, in every project.
    pub(super) fn work(&self, cx: &App) -> Work {
        let statuses = self.projects.iter().flat_map(|p| p.read(cx).sessions.iter()).map(|s| s.read(cx).status.clone());
        statuses.fold(Work::default(), |work, status| match status {
            SessionStatus::Working => Work::new(work.working + 1, work.needs_you),
            status if status.needs_you() => Work::new(work.working, work.needs_you + 1),
            _ => work,
        })
    }

    /// The numbers the bar shows, and the two loops that keep them: the machine each second, the providers each minute.
    pub(super) fn start_vitals(cx: &mut Context<Self>) -> (Entity<Vitals>, Vec<Task<()>>) {
        let vitals = cx.new(|_| Vitals::new(Box::new(SysinfoProbe::new())));
        let load = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(LOAD_EVERY).await;
                if this.update(cx, |shell, cx| shell.sample_vitals(cx)).is_err() {
                    break;
                }
            }
        });
        let providers = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FIRST_ASK).await;
            loop {
                let Ok(job) = this.update(cx, |shell, cx| shell.provider_job(cx)) else { break };
                if let Some((host, openrouter, secrets)) = job {
                    let answers = cx
                        .background_executor()
                        .spawn(async move {
                            let key = openrouter.then(|| secrets.read(OPENROUTER_KEY).ok().flatten()).flatten();
                            Vitals::ask(&Vitals::sources(key), host.as_ref(), unix_now())
                        })
                        .await;
                    let settled = this.update(cx, |shell, cx| {
                        shell.vitals.update(cx, |vitals, cx| {
                            for (name, lead, answer) in answers {
                                vitals.settle(&name, lead, answer);
                            }
                            cx.notify();
                        })
                    });
                    if settled.is_err() {
                        break;
                    }
                }
                cx.background_executor().timer(PROVIDERS_EVERY).await;
            }
        });
        (vitals, vec![load, providers])
    }

    pub(super) fn sample_vitals(&mut self, cx: &mut Context<Self>) {
        let work = self.work(cx);
        self.vitals.update(cx, |vitals, cx| {
            vitals.sample();
            vitals.set_work(work);
            cx.notify();
        });
    }

    /// The job of the next asking, on the front project's host. The key itself is read off this thread.
    fn provider_job(&self, cx: &App) -> Option<Job> {
        let host = self.active()?.read(cx).host();
        Some((host, providers::default_choice(cx) == Choice::OpenRouter, providers::secrets(cx)))
    }

    /// The bar, once a project is open: the start screen and Settings have none.
    pub(super) fn footer(&self) -> Option<AnyElement> {
        (self.active().is_some() && self.settings.is_none()).then(|| {
            // The bar starts where the panes start, past the rail and the panels' gap, and ends where they end.
            div().flex_none().overflow_hidden().pl(gpui_kit::px(self.footer_plan.0)).pr(gpui_kit::px(8.)).pt(gpui_kit::px(super::types::PANE_GAP)).pb(gpui_kit::px(8.)).debug_selector(|| "footer".into()).child(self.vitals.clone()).into_any_element()
        })
    }
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}
