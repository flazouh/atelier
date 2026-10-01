use std::collections::HashMap;

use beui::{checks_panel::{CheckState as Ui, fault}, };
use atelier_forge::{CheckStatus, Conclusion, Job, JobRef, RepoRef, Step};

use crate::{
    checks::{JobLog, check_runs, split_log, state_of, steps, wants_log},
    fixture::sample,
};

fn job(steps: Vec<(&str, CheckStatus, Option<Conclusion>)>) -> Job {
    let reference = JobRef { repo: RepoRef::new("github.com", "o", "r"), id: 7 };
    Job {
        reference,
        name: "linux".into(),
        status: CheckStatus::Done,
        conclusion: Some(Conclusion::Failure),
        run_id: 1,
        attempt: 1,
        steps: steps
            .into_iter()
            .enumerate()
            .map(|(i, (name, status, conclusion))| Step { number: i as u32 + 1, name: name.into(), status, conclusion, started_at: Some(10), completed_at: Some(15) })
            .collect(),
    }
}

const LOG: &str = "\u{feff}2026-09-29T04:00:00.0000001Z Current runner version: '2.337.0'\n\
2026-09-29T04:00:01.0000001Z Complete job name: linux\n\
2026-09-29T04:00:02.0000001Z ##[group]Run actions/checkout@v4\n\
2026-09-29T04:00:02.0000002Z with: fetch-depth: 1\n\
2026-09-29T04:00:03.0000001Z ##[group]Run cargo test\n\
2026-09-29T04:00:04.0000001Z running 3 tests\n\
2026-09-29T04:00:05.0000001Z error[E0308]: mismatched types\n\
2026-09-29T04:00:05.0000002Z   --> src/request.rs:22:37\n\
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 101.\n\
2026-09-29T04:00:07.0000001Z Post job cleanup.\n\
2026-09-29T04:00:08.0000001Z Cleaning up orphan processes\n";

#[test]
fn a_log_is_cut_where_the_runner_starts_each_step() {
    let segments = split_log(LOG);
    assert_eq!(segments.len(), 5);
    assert!(segments[0][0].contains("Current runner version") && segments[0].len() == 2);
    assert!(segments[2][0].contains("Run cargo test") && segments[2].len() == 5);
    assert!(segments[3][0].contains("Post job cleanup."));
    assert!(segments[4][0].contains("Cleaning up orphan"));
    assert!(split_log("").is_empty());
    assert_eq!(split_log("just one line").len(), 1);
}

#[test]
fn only_the_failing_step_keeps_its_lines_and_the_fault_names_the_cause() {
    let job = job(vec![
        ("Set up job", CheckStatus::Done, Some(Conclusion::Success)),
        ("Run actions/checkout@v4", CheckStatus::Done, Some(Conclusion::Success)),
        ("Run cargo test", CheckStatus::Done, Some(Conclusion::Failure)),
        ("Post Run actions/checkout@v4", CheckStatus::Done, Some(Conclusion::Success)),
        ("Complete job", CheckStatus::Done, Some(Conclusion::Success)),
    ]);
    let steps = steps(&job, LOG);
    assert_eq!(steps.len(), 5);
    assert!(steps.iter().enumerate().all(|(i, s)| (i == 2) == !s.log.is_empty()), "{steps:#?}");
    assert_eq!(steps[2].state, Ui::Failed);
    assert_eq!(steps[2].seconds, Some(5));
    let run = beui::CheckRun { name: "linux".into(), summary: "".into(), state: Ui::Failed, steps };
    let fault = fault(&run).unwrap();
    assert_eq!(fault.step.as_ref(), "Run cargo test");
    assert!(fault.line.contains("error[E0308]"), "{}", fault.line);
}

#[test]
fn when_steps_and_parts_do_not_line_up_the_failing_step_takes_the_part_with_the_error() {
    let job = job(vec![("Set up job", CheckStatus::Done, Some(Conclusion::Success)), ("Run cargo test", CheckStatus::Done, Some(Conclusion::Failure))]);
    let steps = steps(&job, LOG);
    assert!(steps[1].log.iter().any(|l| l.contains("##[error]")));
    assert!(steps[0].log.is_empty());
    let none = super::checks::steps(&job, "no markers, no errors\n");
    assert!(none[1].log.is_empty(), "no part with an error: no lines rather than the wrong ones");
}

#[test]
fn a_check_is_passed_failed_running_queued_or_skipped() {
    use CheckStatus::*;
    use Conclusion::*;
    let cases = [
        (Queued, None, Ui::Queued),
        (Running, None, Ui::Running),
        (Done, Some(Success), Ui::Passed),
        (Done, Some(Neutral), Ui::Passed),
        (Done, Some(Skipped), Ui::Skipped),
        (Done, Some(Failure), Ui::Failed),
        (Done, Some(TimedOut), Ui::Failed),
        (Done, Some(Cancelled), Ui::Failed),
        (Done, Some(ActionRequired), Ui::Failed),
        (Done, Some(Stale), Ui::Failed),
        (Done, None, Ui::Running),
    ];
    for (status, conclusion, want) in cases {
        assert_eq!(state_of(status, conclusion), want, "{status:?} {conclusion:?}");
    }
}

#[test]
fn only_a_failing_check_with_a_job_asks_for_its_log() {
    let mut failing = sample::check("linux", CheckStatus::Done, Some(Conclusion::Failure));
    assert!(wants_log(&failing).is_none(), "a check from another app has no job");
    failing.job = Some(JobRef { repo: RepoRef::new("github.com", "o", "r"), id: 7 });
    assert_eq!(wants_log(&failing).map(|j| j.id), Some(7));
    let mut passing = sample::check("lint", CheckStatus::Done, Some(Conclusion::Success));
    passing.job = failing.job.clone();
    assert!(wants_log(&passing).is_none());
}

#[test]
fn the_rows_carry_the_steps_of_the_jobs_read_so_far() {
    let mut failing = sample::check("linux", CheckStatus::Done, Some(Conclusion::Failure));
    failing.job = Some(JobRef { repo: RepoRef::new("github.com", "o", "r"), id: 7 });
    let other = sample::check("lint", CheckStatus::Done, Some(Conclusion::Success));
    let mut jobs = HashMap::new();
    let rows = check_runs(&[failing.clone(), other.clone()], &jobs);
    assert_eq!(rows.len(), 2);
    assert!(rows[0].steps.is_empty(), "the log has not been read yet");
    jobs.insert(7, JobLog { job: job(vec![("Run cargo test", CheckStatus::Done, Some(Conclusion::Failure))]), log: "##[group]Run cargo test\nerror: boom\n".into() });
    let rows = check_runs(&[failing, other], &jobs);
    assert_eq!(rows[0].steps.len(), 1);
    assert!(fault(&rows[0]).unwrap().line.contains("boom"));
    assert!(rows[1].steps.is_empty());
}

#[test]
fn the_recorded_log_of_a_real_job_splits_into_the_steps_the_runner_reported() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../forge/tests/fixtures/github");
    let log = std::fs::read_to_string(format!("{dir}/GET-repos-oven-sh-bun-actions-jobs-109256890058-logs.json")).unwrap();
    let recorded: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/GET-repos-oven-sh-bun-actions-jobs-109256890058.json")).unwrap()).unwrap();
    let segments = split_log(&log);
    assert_eq!(segments.len(), recorded["steps"].as_array().unwrap().len(), "one part for each of the job's steps");
    assert!(segments[1][0].contains("##[group]Run actions/github-script"));
    assert!(segments[2][0].contains("Cleaning up orphan processes"));
}

#[test]
fn a_job_that_failed_in_a_run_that_succeeded_was_allowed_to_fail() {
    use crate::checks::tolerated_or;
    use atelier_forge::RunInfo;
    let run = |suite| Some(RunInfo { id: 1, workflow: "ci".into(), number: 3, event: "push".into(), suite });
    let mut allowed = sample::check("flaky", CheckStatus::Done, Some(Conclusion::Failure));
    allowed.run = run(Some(Conclusion::Success));
    assert_eq!(tolerated_or(&allowed), Ui::Tolerated, "failed job, succeeded run");
    let mut broken = sample::check("tests", CheckStatus::Done, Some(Conclusion::Failure));
    broken.run = run(Some(Conclusion::Failure));
    assert_eq!(tolerated_or(&broken), Ui::Failed, "the run failed too: this is why it is red");
    let mut unknown = sample::check("tests", CheckStatus::Done, Some(Conclusion::Failure));
    unknown.run = run(None);
    assert_eq!(tolerated_or(&unknown), Ui::Failed, "a run with no known ending is not a reason to excuse a failure");
    let bare = sample::check("status", CheckStatus::Done, Some(Conclusion::Failure));
    assert_eq!(tolerated_or(&bare), Ui::Failed, "a commit status has no run");
    let passed = sample::check("ok", CheckStatus::Done, Some(Conclusion::Success));
    assert_eq!(tolerated_or(&passed), Ui::Passed);
}

#[test]
fn a_tolerated_job_still_has_its_fault_read_and_shown_apart() {
    use atelier_forge::RunInfo;
    let mut allowed = sample::check("flaky", CheckStatus::Done, Some(Conclusion::Failure));
    allowed.job = Some(JobRef { repo: RepoRef::new("github.com", "o", "r"), id: 7 });
    allowed.run = Some(RunInfo { id: 1, workflow: "ci".into(), number: 3, event: "push".into(), suite: Some(Conclusion::Success) });
    assert!(wants_log(&allowed).is_some(), "its log is read for its Fault");
    let rows = check_runs(&[allowed], &HashMap::new());
    assert_eq!(rows[0].state, Ui::Tolerated);
}
