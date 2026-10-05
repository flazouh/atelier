use atelier_forge::{ChangedFile, Change, Check, CheckStatus, Conclusion};
use atelier_ui::{PrChipData, PrState, pr_glance::PrFailing};

use super::helpers::{failing_check, files_of, fix_prompt};

fn check(name: &str, status: CheckStatus, conclusion: Option<Conclusion>) -> Check {
    Check {
        name: name.into(),
        status,
        conclusion,
        url: None,
        required: false,
        started_at: None,
        completed_at: None,
        job: None,
        run: None,
    }
}

fn pr() -> PrChipData {
    PrChipData { number: 3311, repo: "o/r".into(), title: "t".into(), state: PrState::Open, url: "u".into(), facts: None }
}

#[test]
fn the_failing_check_is_the_first_that_failed_and_finished() {
    let checks = [
        check("build", CheckStatus::Done, Some(Conclusion::Success)),
        check("lint", CheckStatus::Running, Some(Conclusion::Failure)),
        check("test", CheckStatus::Done, Some(Conclusion::Failure)),
        check("e2e", CheckStatus::Done, Some(Conclusion::TimedOut)),
    ];
    assert_eq!(failing_check(&checks).map(|c| c.name.as_str()), Some("test"));
    assert!(failing_check(&checks[..1]).is_none());
    assert!(failing_check(&[check("skip", CheckStatus::Done, Some(Conclusion::Skipped))]).is_none(), "a skipped check did not fail");
}

#[test]
fn the_card_lists_the_three_biggest_files() {
    let file = |path: &str, additions, deletions| ChangedFile { path: path.into(), additions, deletions, change: Change::Modified };
    let files = files_of(vec![file("a", 1, 0), file("b", 30, 30), file("c", 5, 5), file("d", 0, 80)]);
    let paths: Vec<_> = files.iter().map(|f| f.path.as_ref()).collect();
    assert_eq!(paths, ["d", "b", "c"]);
    assert_eq!((files[0].added, files[0].removed), (0, 80));
}

#[test]
fn the_fix_prompt_names_the_check_its_line_its_log_and_the_branch() {
    let failing = PrFailing { name: "test".into(), line: Some("error[E0308]: mismatched types".into()), url: Some("https://x/log".into()) };
    let text = fix_prompt(&pr(), &failing, Some("relay-abort"));
    assert!(text.starts_with("The check \"test\" fails on o/r#3311:"));
    assert!(text.contains("error[E0308]: mismatched types"));
    assert!(text.contains("https://x/log"));
    assert!(text.ends_with("push the fix to relay-abort."));
    let bare = fix_prompt(&pr(), &PrFailing { line: None, url: None, ..failing }, None);
    assert_eq!(bare, "The check \"test\" fails on o/r#3311.\n\nRead the log, find the cause, fix it.");
}
