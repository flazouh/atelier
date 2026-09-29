//! The checks as beui draws them, with the Fault of a failing one. The forge lists a job's steps and its
//! log apart; the log is one text for the whole job. A step's part of it is found by the markers the runner
//! writes: each `Run` step opens with `##[group]Run ...`, each post step with `Post job cleanup.`, and the
//! job ends with `Cleaning up orphan processes`. Only failing steps keep their lines, so a job of thousands
//! of lines costs what its failure costs.
use std::collections::HashMap;

use beui::{
    CheckRun, JobStep,
    checks_panel::CheckState as UiState,
};
use gpui_kit::SharedString;
use lathe_forge::{Check, CheckStatus, Conclusion, Job, JobRef};

/// A check in the panel's words. Cancelled, timed out, action required and stale count as failing, as
/// they do for the merge (`present::merge_facts`).
pub fn state_of(status: CheckStatus, conclusion: Option<Conclusion>) -> UiState {
    match (status, conclusion) {
        (CheckStatus::Queued, _) => UiState::Queued,
        (CheckStatus::Running, _) | (CheckStatus::Done, None) => UiState::Running,
        (CheckStatus::Done, Some(c)) => match c {
            Conclusion::Success | Conclusion::Neutral => UiState::Passed,
            Conclusion::Skipped => UiState::Skipped,
            Conclusion::Failure | Conclusion::TimedOut | Conclusion::Cancelled | Conclusion::ActionRequired | Conclusion::Stale => UiState::Failed,
        },
    }
}

/// The check's state, with a failure the workflow allowed (the job failed, the run around it succeeded) told
/// apart: it is Tolerated, and never why the pull request is red.
pub fn tolerated_or(check: &Check) -> UiState {
    let state = state_of(check.status, check.conclusion);
    let run_ok = check.run.as_ref().and_then(|r| r.suite).is_some_and(|c| matches!(c, Conclusion::Success | Conclusion::Neutral));
    if state == UiState::Failed && run_ok { UiState::Tolerated } else { state }
}

/// A failing check whose job can be read, and so whose Fault can be shown.
pub fn wants_log(check: &Check) -> Option<&JobRef> {
    let failing = matches!(tolerated_or(check), UiState::Failed | UiState::Tolerated);
    check.job.as_ref().filter(|_| failing)
}

/// How many failing jobs are read at once. More than this and the rest say only that they failed.
pub const MAX_LOGS: usize = 8;

/// Everything read about a job: its steps and its log.
#[derive(Clone, Debug, PartialEq)]
pub struct JobLog {
    pub job: Job,
    pub log: String,
}

/// The checks as rows. `jobs` are the jobs read so far, by id.
pub fn check_runs(checks: &[Check], jobs: &HashMap<u64, JobLog>) -> Vec<CheckRun> {
    checks
        .iter()
        .map(|check| {
            let state = tolerated_or(check);
            let read = check.job.as_ref().and_then(|j| jobs.get(&j.id));
            let summary = check.run.as_ref().map(|run| run.workflow.clone()).unwrap_or_default();
            CheckRun { name: check.name.clone().into(), summary: summary.into(), state, steps: read.map(|j| steps(&j.job, &j.log)).unwrap_or_default() }
        })
        .collect()
}

/// A job's steps, the failing ones with their lines.
pub fn steps(job: &Job, log: &str) -> Vec<JobStep> {
    let segments = split_log(log);
    let n = job.steps.len();
    // The steps and the segments line up when there is one segment for each, which is the runner's
    // layout. When they do not, the failing step takes the segment that holds an error.
    let aligned = segments.len() == n;
    let error_segment = segments.iter().position(|s| s.iter().any(|l| is_error(l)));
    job.steps
        .iter()
        .enumerate()
        .map(|(at, step)| {
            let state = state_of(step.status, step.conclusion);
            let failed = state == UiState::Failed;
            let lines: &[&str] = match (failed, aligned, error_segment) {
                (false, ..) => &[],
                (true, true, _) => &segments[at],
                (true, false, Some(i)) => &segments[i],
                (true, false, None) => &[],
            };
            let seconds = step.started_at.zip(step.completed_at).map(|(a, b)| b.saturating_sub(a));
            JobStep { name: step.name.clone().into(), state, seconds, log: lines.iter().map(|l| SharedString::from(l.to_string())).collect() }
        })
        .collect()
}

fn is_error(line: &str) -> bool {
    line.contains("##[error]")
}

/// The runner's own markers, after the timestamp.
fn body(line: &str) -> &str {
    let line = line.strip_prefix('\u{feff}').unwrap_or(line);
    match line.split_once(' ') {
        Some((stamp, rest)) if stamp.len() > 20 && stamp.ends_with('Z') && stamp.as_bytes()[4] == b'-' => rest,
        _ => line,
    }
}

fn starts_step(body: &str) -> bool {
    body.starts_with("##[group]Run ") || body.starts_with("Post job cleanup.") || body.starts_with("Cleaning up orphan processes")
}

/// The log cut into the parts the runner wrote for each step: the setup before the first `Run`, then one
/// part from each opening marker to the next. Lines keep their timestamps (the panel cleans them).
pub fn split_log(log: &str) -> Vec<Vec<&str>> {
    let mut segments: Vec<Vec<&str>> = vec![Vec::new()];
    for line in log.lines() {
        if starts_step(body(line)) && !segments.last().is_some_and(Vec::is_empty) {
            segments.push(Vec::new());
        }
        if let Some(last) = segments.last_mut() {
            last.push(line);
        }
    }
    if segments.len() == 1 && segments[0].is_empty() {
        segments.clear();
    }
    segments
}
