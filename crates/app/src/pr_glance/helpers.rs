use std::{sync::Arc, time::Instant};

use atelier_forge::{
    ChangedFile, Check, CheckStatus, Conclusion, Forge, ForgeResult, JobRef, MergeOutcome, MergeRequest, Pull, PullRef, PullState,
    Verdict, present,
};
use atelier_ui::{
    PrChipData, PrParts, PrPart,
    pr_glance::{PrAction, PrDoing, PrFailing, PrFile, PrSession, key_of, pr_cards, top_files},
};
use gpui_kit::{App, AppContext, Context, Entity, WeakEntity, Window};

use crate::open_project::{OpenProject, ProjectEvent};
use super::structs::Projects;
use super::types::LIVE_EVERY;

/// Makes the cards read and act through the open projects, with the parts the reader hid in `hidden`
/// hidden, and keeps what the reader hides from now on.
pub fn install(hidden: &[String], cx: &mut App) {
    let store = pr_cards(cx);
    let mut kept = PrParts::without(hidden);
    store.update(cx, |s, cx| {
        s.set_parts(kept, cx);
        s.on_open(opened);
        s.on_action(act);
    });
    cx.observe(&store, move |store, cx| {
        let parts = store.read(cx).parts();
        if parts != kept {
            kept = parts;
            let off = parts.hidden();
            crate::settings_pane::save(cx, move |s| s.pr_card_off = off);
        }
    })
    .detach();
}

/// A card of `pr` opened, or closed: the project of its repository starts its reads, or stops them.
pub(crate) fn opened(pr: &PrChipData, open: bool, cx: &mut App) {
    let Some(project) = project_for(&pr.repo, cx) else { return };
    let pr = pr.clone();
    project.update(cx, |p, cx| {
        if open {
            start(p, pr, cx);
        } else {
            p.glance.open.remove(&pr.number);
        }
    });
}

/// Lets cards of `project`'s repository read through it.
pub fn register(project: WeakEntity<OpenProject>, cx: &mut App) {
    let projects = &mut cx.default_global::<Projects>().0;
    projects.retain(|p| p.upgrade().is_some());
    projects.push(project);
}

fn project_for(repo: &str, cx: &App) -> Option<Entity<OpenProject>> {
    let projects = cx.try_global::<Projects>()?;
    projects.0.iter().filter_map(WeakEntity::upgrade).find(|p| p.read(cx).repo.as_ref().is_some_and(|r| r.slug() == repo))
}

fn reference(p: &OpenProject, pr: &PrChipData) -> Option<PullRef> {
    Some(PullRef { repo: p.repo.clone()?, number: pr.number })
}

/// The first check that failed, as the card names it.
pub(super) fn failing_check(checks: &[Check]) -> Option<&Check> {
    checks.iter().find(|c| {
        c.status == CheckStatus::Done
            && matches!(c.conclusion, Some(Conclusion::Failure | Conclusion::TimedOut | Conclusion::ActionRequired))
    })
}

pub(super) fn files_of(files: Vec<ChangedFile>) -> Vec<PrFile> {
    top_files(files, |f| f.additions + f.deletions)
        .into_iter()
        .map(|f| PrFile { path: f.path.into(), added: f.additions, removed: f.deletions })
        .collect()
}

/// What the agent is asked when the reader presses Ask the agent to fix.
pub(super) fn fix_prompt(pr: &PrChipData, failing: &PrFailing, head: Option<&str>) -> String {
    let mut text = format!("The check \"{}\" fails on {}#{}", failing.name, pr.repo, pr.number);
    match &failing.line {
        Some(line) => text.push_str(&format!(":\n\n    {line}\n")),
        None => text.push_str(".\n"),
    }
    if let Some(url) = &failing.url {
        text.push_str(&format!("\nIts log: {url}\n"));
    }
    text.push_str("\nRead the log, find the cause, fix it");
    match head {
        Some(head) if !head.is_empty() => text.push_str(&format!(", and push the fix to {head}.")),
        _ => text.push('.'),
    }
    text
}

fn linked_session(p: &OpenProject, reference: &PullRef, cx: &App) -> Option<Entity<crate::agent_session::AgentSession>> {
    p.sessions.iter().find(|s| s.read(cx).reviews.pull.as_ref() == Some(reference)).cloned()
}

fn session_of(p: &OpenProject, reference: &PullRef, cx: &App) -> Option<PrSession> {
    let session = linked_session(p, reference, cx)?;
    let s = session.read(cx);
    Some(PrSession {
        title: s.title.clone(),
        status: s.status.words(),
        running: matches!(s.status, atelier_ui::session_status::SessionStatus::Working),
    })
}

fn say(pr: &PrChipData, doing: Option<PrDoing>, cx: &mut App) {
    pr_cards(cx).update(cx, |s, cx| s.update_glance(key_of(pr), |g| g.doing = doing, cx));
}

type Read = (ForgeResult<Pull>, ForgeResult<Vec<Check>>, Option<ForgeResult<Vec<ChangedFile>>>);

/// Reads `pr` for its open card, in parallel and off the UI thread, then again every `LIVE_EVERY` while the
/// card stays open and its Live part shows.
fn start(p: &mut OpenProject, pr: PrChipData, cx: &mut Context<OpenProject>) {
    let Some(reference) = reference(p, &pr) else { return };
    let forge = p.chip_forge();
    let session = session_of(p, &reference, cx);
    pr_cards(cx).update(cx, |s, cx| s.update_glance(key_of(&pr), |g| g.session = session, cx));
    let number = pr.number;
    let reading = cx.spawn(async move |this, cx| {
        let mut size: Option<(u32, u32)> = None;
        loop {
            let started = Instant::now();
            let (f, r) = (forge.clone(), reference.clone());
            let pull = cx.background_spawn(async move { f.pull(&r) });
            let (f, r) = (forge.clone(), reference.clone());
            let checks = cx.background_spawn(async move { f.checks(&r) });
            // The files change only with the size: the first read asks beside the others, later ones after.
            let files = size.is_none().then(|| {
                let (f, r) = (forge.clone(), reference.clone());
                cx.background_spawn(async move { f.files(&r) })
            });
            let (pull, checks) = (pull.await, checks.await);
            let files = match files {
                Some(files) => Some(files.await),
                None => match &pull {
                    Ok(pull) if size != Some((pull.additions, pull.deletions)) => {
                        let (f, r) = (forge.clone(), reference.clone());
                        Some(cx.background_spawn(async move { f.files(&r) }).await)
                    }
                    _ => None,
                },
            };
            if let Ok(pull) = &pull {
                size = Some((pull.additions, pull.deletions));
            }
            if crate::timings::enabled() {
                eprintln!("pr card #{number}: read in {} ms", started.elapsed().as_millis());
            }
            let Ok((live, log)) = this.update(cx, |p, cx| got(p, &pr, (pull, checks, files), cx)) else { break };
            if let Some(job) = log {
                read_log(&this, &forge, &pr, job, cx).await;
            }
            if !live {
                break;
            }
            cx.background_executor().timer(LIVE_EVERY).await;
        }
    });
    p.glance.open.insert(number, reading);
}

/// Reads a failed job's log for its first error line, and puts it under the card's failing check.
async fn read_log(this: &WeakEntity<OpenProject>, forge: &Arc<dyn Forge>, pr: &PrChipData, job: JobRef, cx: &mut gpui_kit::AsyncApp) {
    let (id, forge) = (job.id, forge.clone());
    let line = cx.background_spawn(async move { forge.job_log(&job).ok().and_then(|log| atelier_forge::log::first_error_line(&log)) }).await;
    _ = this.update(cx, |p, cx| {
        p.glance.lines.insert(id, line.clone());
        pr_cards(cx).update(cx, |s, cx| {
            s.update_glance(key_of(pr), |g| {
                if let Some(failing) = &mut g.failing {
                    failing.line = line.map(Into::into);
                }
            }, cx)
        });
    });
}

/// Puts a read into the card. Says whether to read again, and the failed job whose log to read.
fn got(p: &mut OpenProject, pr: &PrChipData, (pull, checks, files): Read, cx: &mut Context<OpenProject>) -> (bool, Option<JobRef>) {
    let reference = reference(p, pr);
    let session = reference.as_ref().and_then(|r| session_of(p, r, cx));
    let failing = checks.as_ref().ok().and_then(|c| failing_check(c)).map(|c| {
        let line = c.job.as_ref().and_then(|j| p.glance.lines.get(&j.id).cloned().flatten());
        (PrFailing { name: c.name.clone().into(), line: line.map(Into::into), url: c.url.clone().map(Into::into) }, c.job.clone())
    });
    let log = failing.as_ref().and_then(|(_, job)| job.clone()).filter(|j| !p.glance.lines.contains_key(&j.id));
    let open = pull.as_ref().map_or(true, |pull| matches!(pull.state, PullState::Open | PullState::Draft));
    let facts = pull.as_ref().ok().and_then(|pull| present::chip_of(&pull.summary()).facts);
    if let Ok(pull) = pull {
        p.glance.pulls.insert(pr.number, pull);
    }
    let store = pr_cards(cx);
    let live = store.read(cx).parts().shows(PrPart::Live) && open;
    store.update(cx, |s, cx| {
        s.update_glance(key_of(pr), |g| {
            if facts.is_some() {
                g.facts = facts;
            }
            if let Some(Ok(files)) = files {
                g.files = Some(files_of(files));
            }
            if checks.is_ok() {
                g.failing = failing.map(|(f, _)| f);
            }
            g.session = session;
            g.read_at = Some(Instant::now());
        }, cx)
    });
    (live, log)
}

fn act(action: PrAction, pr: &PrChipData, window: &mut Window, cx: &mut App) {
    if action == PrAction::OpenLog {
        let url = pr_cards(cx).read(cx).glance(&key_of(pr)).and_then(|g| g.failing.as_ref()?.url.clone());
        if let Some(url) = url {
            cx.open_url(&url);
        }
        return;
    }
    let Some(project) = project_for(&pr.repo, cx) else { return };
    let pr = pr.clone();
    project.update(cx, |p, cx| {
        let Some(reference) = reference(p, &pr) else { return };
        match action {
            PrAction::Merge => merge(p, pr, reference, cx),
            PrAction::Approve => {
                let forge = p.chip_forge();
                write(pr, "Approving…", cx.background_spawn(async move { forge.submit_review(&reference, Verdict::Approve, "").map(|()| "Approved".to_string()) }), cx);
            }
            PrAction::AskToFix => ask_to_fix(p, &pr, &reference, window, cx),
            PrAction::OpenSession => {
                if let Some(session) = linked_session(p, &reference, cx) {
                    cx.emit(ProjectEvent::ShowSession(session));
                }
            }
            PrAction::OpenLog => {}
        }
    });
}

fn merge(p: &mut OpenProject, pr: PrChipData, reference: PullRef, cx: &mut Context<OpenProject>) {
    let Some(pull) = p.glance.pulls.get(&pr.number) else {
        return say(&pr, Some(PrDoing::Failed("The card has not read the pull request yet.".into())), cx);
    };
    let request = MergeRequest {
        method: pull.merge.default_method,
        title: None,
        message: None,
        expected_head: Some(pull.head_sha.clone()),
        when_ready: false,
        delete_branch: pull.merge.delete_branch_on_merge,
    };
    let forge = p.chip_forge();
    let merging = cx.background_spawn(async move {
        forge.merge(&reference, &request).map(|outcome| {
            match outcome {
                MergeOutcome::Merged => "Merged",
                MergeOutcome::WillMergeWhenReady => "Set to merge when ready",
                MergeOutcome::Queued => "Put in the merge queue",
            }
            .to_string()
        })
    });
    write(pr, "Merging…", merging, cx);
}

/// Says `working` on the card while `writing` runs, then what it did, and reads the card again.
fn write(pr: PrChipData, working: &'static str, writing: gpui_kit::Task<ForgeResult<String>>, cx: &mut Context<OpenProject>) {
    say(&pr, Some(PrDoing::Working(working.into())), cx);
    cx.spawn(async move |this, cx| {
        let done = writing.await;
        _ = this.update(cx, |p, cx| {
            let doing = match done {
                Ok(done) => PrDoing::Done(done.into()),
                Err(error) => PrDoing::Failed(error.to_string().into()),
            };
            say(&pr, Some(doing), cx);
            if p.glance.open.contains_key(&pr.number) {
                start(p, pr, cx);
            }
        });
    })
    .detach();
}

/// Asks the session that made the pull request to fix its failing check, or a new session when none did.
fn ask_to_fix(p: &mut OpenProject, pr: &PrChipData, reference: &PullRef, window: &mut Window, cx: &mut Context<OpenProject>) {
    let failing = pr_cards(cx).read(cx).glance(&key_of(pr)).and_then(|g| g.failing.clone());
    let Some(failing) = failing else { return };
    let head = p.glance.pulls.get(&pr.number).map(|pull| pull.head.clone());
    let text = fix_prompt(pr, &failing, head.as_deref());
    let session = match linked_session(p, reference, cx) {
        Some(session) => session,
        None => p.open_session(None, None, window, cx),
    };
    session.update(cx, |s, cx| s.send(text, cx));
    cx.emit(ProjectEvent::ShowSession(session));
    say(pr, Some(PrDoing::Done("Asked the agent".into())), cx);
}
