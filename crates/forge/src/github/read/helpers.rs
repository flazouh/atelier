use super::super::wire::{
    CommentNode,
    CommitNode,
    ContextNode,
    Contexts,
    FileNode,
    Login,
    PullNode,
    Repo,
    RestJob,
    SearchHit,
    StateCount,
    ThreadNode,
};
use crate::{
    Author,
    Change,
    ChangedFile,
    Check,
    CheckCounts,
    CheckStatus,
    Comment,
    Conclusion,
    ForgeError,
    ForgeResult,
    Job,
    JobRef,
    MergeMethod,
    MergeSettings,
    MergeState,
    Opinion,
    Pull,
    PullBrief,
    PullId,
    PullRef,
    PullState,
    PullSummary,
    QueuePlace,
    RepoRef,
    Repository,
    ReviewDecision,
    Reviewer,
    Rights,
    RunInfo,
    Side,
    Step,
    Thread,
    ThreadId,
    Verdict,
    time,
};
use super::types::HOST;

pub(in super::super) fn repo_ref(name_with_owner: &str) -> ForgeResult<RepoRef> {
    let (owner, name) = name_with_owner
        .split_once('/')
        .ok_or_else(|| ForgeError::Unexpected(format!("`{name_with_owner}` is not owner/name")))?;
    Ok(RepoRef::new(HOST, owner, name))
}

fn merge_method(name: &str) -> Option<MergeMethod> {
    match name {
        "MERGE" => Some(MergeMethod::Merge),
        "SQUASH" => Some(MergeMethod::Squash),
        "REBASE" => Some(MergeMethod::Rebase),
        _ => None,
    }
}

fn merge_settings(repo: &Repo) -> MergeSettings {
    let allowed = [
        (repo.merge_commit_allowed, MergeMethod::Merge),
        (repo.squash_merge_allowed, MergeMethod::Squash),
        (repo.rebase_merge_allowed, MergeMethod::Rebase),
    ];
    let methods: Vec<MergeMethod> = allowed.into_iter().filter_map(|(on, method)| on.then_some(method)).collect();
    let asked = repo.viewer_default_merge_method.as_deref().and_then(merge_method);
    let default_method = asked.filter(|m| methods.contains(m)).or_else(|| methods.first().copied()).unwrap_or(MergeMethod::Merge);
    MergeSettings {
        methods,
        default_method,
        auto_merge_allowed: repo.auto_merge_allowed,
        delete_branch_on_merge: repo.delete_branch_on_merge,
        has_queue: repo.merge_queue.as_ref().is_some_and(|queue| !queue.is_null()),
    }
}

/// Whether the reader can push here: `WRITE` and above.
fn can_write(permission: Option<&str>) -> bool {
    matches!(permission, Some("WRITE" | "MAINTAIN" | "ADMIN"))
}

pub(in super::super) fn repository(repo: &Repo) -> ForgeResult<Repository> {
    Ok(Repository {
        id: repo.id.clone(),
        reference: repo_ref(&repo.name_with_owner)?,
        url: repo.url.clone().unwrap_or_else(|| format!("https://{HOST}/{}", repo.name_with_owner)),
        default_branch: repo.default_branch_ref.as_ref().map(|b| b.name.clone()),
        merge: merge_settings(repo),
        can_write: can_write(repo.viewer_permission.as_deref()),
    })
}

fn pull_state(node_state: &str, draft: bool, merged: bool) -> PullState {
    if merged || node_state == "MERGED" {
        PullState::Merged
    } else if node_state == "CLOSED" {
        PullState::Closed
    } else if draft {
        PullState::Draft
    } else {
        PullState::Open
    }
}

fn merge_state(text: Option<&str>) -> MergeState {
    match text {
        Some("CLEAN" | "HAS_HOOKS") => MergeState::Clean,
        Some("UNSTABLE") => MergeState::Unstable,
        Some("BLOCKED") => MergeState::Blocked,
        Some("BEHIND") => MergeState::Behind,
        Some("DIRTY") => MergeState::Dirty,
        Some("DRAFT") => MergeState::Draft,
        _ => MergeState::Unknown,
    }
}

fn decision(text: Option<&str>) -> ReviewDecision {
    match text {
        Some("APPROVED") => ReviewDecision::Approved,
        Some("CHANGES_REQUESTED") => ReviewDecision::ChangesRequested,
        Some("REVIEW_REQUIRED") => ReviewDecision::Required,
        _ => ReviewDecision::NotRequired,
    }
}

fn verdict(state: &str) -> Option<Verdict> {
    match state {
        "APPROVED" => Some(Verdict::Approve),
        "CHANGES_REQUESTED" => Some(Verdict::RequestChanges),
        "COMMENTED" => Some(Verdict::Comment),
        _ => None,
    }
}

/// How the counts of check runs and commit statuses split into passed, failed and running.
fn counts(contexts: &Contexts) -> CheckCounts {
    let mut counts = CheckCounts::default();
    let states = contexts.check_run_counts_by_state.iter().chain(&contexts.status_context_counts_by_state);
    for StateCount { state, count } in states {
        match state.as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" | "COMPLETED" => counts.passed += count,
            "IN_PROGRESS" | "QUEUED" | "PENDING" | "WAITING" | "EXPECTED" | "REQUESTED" => counts.running += count,
            _ => counts.failed += count,
        }
    }
    counts
}

fn last_commit(commits: Option<&super::super::wire::Nodes<CommitNode>>) -> Option<&super::super::wire::CommitInfo> {
    commits?.nodes.iter().flatten().last().map(|node| &node.commit)
}

fn rollup_counts(commits: Option<&super::super::wire::Nodes<CommitNode>>) -> Option<CheckCounts> {
    last_commit(commits)?.status_check_rollup.as_ref().map(|rollup| counts(&rollup.contexts))
}

fn login(author: &Option<Login>) -> String {
    author.as_ref().map_or_else(|| "ghost".to_string(), |a| a.login.clone())
}

pub(in super::super) fn pull(repo: &Repo, node: &PullNode) -> ForgeResult<Pull> {
    let reference = PullRef { repo: repo_ref(&repo.name_with_owner)?, number: node.number };
    let opinions = node
        .latest_reviews
        .iter()
        .flat_map(|reviews| reviews.nodes.iter().flatten())
        .filter_map(|review| {
            Some(Opinion { reviewer: login(&review.author), verdict: verdict(&review.state)? })
        })
        .collect();
    let requested = node
        .review_requests
        .iter()
        .flat_map(|requests| requests.nodes.iter().flatten())
        .filter_map(|request| {
            let who = request.requested_reviewer.as_ref()?;
            match who.typename.as_str() {
                "Team" => who.slug.clone().map(Reviewer::Team),
                _ => who.login.clone().map(Reviewer::Person),
            }
        })
        .collect();
    let rights = if node.viewer_can_merge_as_admin {
        Rights::Bypass
    } else if can_write(repo.viewer_permission.as_deref()) {
        Rights::Merge
    } else {
        Rights::Cannot
    };
    Ok(Pull {
        id: PullId(node.id.clone()),
        reference,
        title: node.title.clone(),
        body: node.body.clone(),
        state: pull_state(&node.state, node.is_draft, node.merged),
        url: node.url.clone(),
        author: login(&node.author),
        base: node.base_ref_name.clone(),
        base_sha: node.base_ref_oid.clone(),
        head: node.head_ref_name.clone(),
        head_sha: node.head_ref_oid.clone(),
        created_at: time::parse(&node.created_at).unwrap_or(0),
        updated_at: time::parse(&node.updated_at).unwrap_or(0),
        additions: node.additions,
        deletions: node.deletions,
        changed_files: node.changed_files,
        remarks: node.comments.total,
        conflicting: node.mergeable.as_deref() == Some("CONFLICTING"),
        merge_state: merge_state(node.merge_state_status.as_deref()),
        review: decision(node.review_decision.as_deref()),
        opinions,
        requested,
        checks: rollup_counts(node.commits.as_ref()).unwrap_or_default(),
        queue: node.is_in_merge_queue.then(|| QueuePlace { position: node.merge_queue_entry.as_ref().and_then(|e| e.position) }),
        auto_merge: node.auto_merge_request.as_ref().is_some_and(|request| !request.is_null()),
        rights,
        can_update: node.viewer_can_update,
        merge: merge_settings(repo),
    })
}

pub(in super::super) fn last_review_point(node: &PullNode) -> Option<String> {
    node.viewer_latest_review.as_ref()?.commit.as_ref().map(|commit| commit.oid.clone())
}

pub(in super::super) fn file(node: &FileNode) -> ChangedFile {
    let change = match node.change_type.as_str() {
        "ADDED" => Change::Added,
        "DELETED" => Change::Deleted,
        "RENAMED" => Change::Renamed,
        "COPIED" => Change::Copied,
        _ => Change::Modified,
    };
    ChangedFile { path: node.path.clone(), additions: node.additions, deletions: node.deletions, change }
}

pub(in super::super) fn comment(node: &CommentNode) -> Comment {
    let (author, kind) = match &node.author {
        Some(who) => (who.login.clone(), if who.typename == "Bot" { Author::Bot } else { Author::Person }),
        None => ("ghost".to_string(), Author::Person),
    };
    Comment {
        id: node.id.clone(),
        author,
        kind,
        body: node.body.clone(),
        created_at: time::parse(&node.created_at).unwrap_or(0),
        // GitHub calls a comment in a review that is not submitted `PENDING`; atelier calls it unsent.
        unsent: node.state.as_deref() == Some("PENDING"),
    }
}

/// A thread with the comments of its first page; the caller fetches the rest when the page says more.
pub(in super::super) fn thread(node: &ThreadNode) -> Thread {
    Thread {
        id: ThreadId(node.id.clone()),
        resolved: node.is_resolved,
        outdated: node.is_outdated,
        path: node.path.clone(),
        file_level: node.subject_type.as_deref() == Some("FILE"),
        line: node.line,
        start_line: node.start_line,
        original_line: node.original_line,
        side: if node.diff_side.as_deref() == Some("LEFT") { Side::Left } else { Side::Right },
        can_resolve: node.viewer_can_resolve,
        can_reply: node.viewer_can_reply,
        comments: node.comments.nodes.iter().flatten().map(comment).collect(),
    }
}

fn status(text: &str) -> CheckStatus {
    match text.to_ascii_lowercase().as_str() {
        "completed" => CheckStatus::Done,
        "in_progress" => CheckStatus::Running,
        _ => CheckStatus::Queued,
    }
}

fn conclusion(text: &str) -> Option<Conclusion> {
    Some(match text.to_ascii_lowercase().as_str() {
        "success" => Conclusion::Success,
        "failure" | "startup_failure" | "error" => Conclusion::Failure,
        "neutral" => Conclusion::Neutral,
        "cancelled" => Conclusion::Cancelled,
        "skipped" => Conclusion::Skipped,
        "timed_out" => Conclusion::TimedOut,
        "action_required" => Conclusion::ActionRequired,
        "stale" => Conclusion::Stale,
        _ => return None,
    })
}

pub(in super::super) fn check(repo: &RepoRef, node: &ContextNode) -> Check {
    if node.typename == "StatusContext" {
        let state = node.state.as_deref().unwrap_or("PENDING");
        let done = matches!(state, "SUCCESS" | "FAILURE" | "ERROR");
        return Check {
            name: node.context.clone().unwrap_or_default(),
            status: if done { CheckStatus::Done } else { CheckStatus::Running },
            conclusion: done.then(|| conclusion(state)).flatten(),
            url: node.target_url.clone(),
            required: node.is_required,
            started_at: node.created_at.as_deref().and_then(time::parse),
            completed_at: None,
            job: None,
            run: None,
        };
    }
    let run = node.check_suite.as_ref().and_then(|suite| suite.workflow_run.as_ref());
    let suite = node.check_suite.as_ref().and_then(|suite| suite.conclusion.as_deref()).and_then(conclusion);
    Check {
        name: node.name.clone().unwrap_or_default(),
        status: status(node.status.as_deref().unwrap_or("QUEUED")),
        conclusion: node.conclusion.as_deref().and_then(conclusion),
        url: node.details_url.clone(),
        required: node.is_required,
        started_at: node.started_at.as_deref().and_then(time::parse),
        completed_at: node.completed_at.as_deref().and_then(time::parse),
        // A job of a workflow run has the id of its check run; other apps' checks have no steps to fetch.
        job: run.and(node.database_id).map(|id| JobRef { repo: repo.clone(), id }),
        run: run.map(|run| RunInfo {
            id: run.database_id,
            workflow: run.workflow.as_ref().map(|w| w.name.clone()).unwrap_or_default(),
            number: run.run_number,
            event: run.event.clone(),
            suite,
        }),
    }
}

pub(in super::super) fn job(repo: &RepoRef, node: &RestJob) -> Job {
    Job {
        reference: JobRef { repo: repo.clone(), id: node.id },
        name: node.name.clone(),
        status: status(&node.status),
        conclusion: node.conclusion.as_deref().and_then(conclusion),
        run_id: node.run_id,
        attempt: node.run_attempt,
        steps: node
            .steps
            .iter()
            .map(|step| Step {
                number: step.number,
                name: step.name.clone(),
                status: status(&step.status),
                conclusion: step.conclusion.as_deref().and_then(conclusion),
                started_at: step.started_at.as_deref().and_then(time::parse),
                completed_at: step.completed_at.as_deref().and_then(time::parse),
            })
            .collect(),
    }
}

/// A search hit that is a pull request, as a chip and as a list row. `None` for a hit that is not one.
pub(in super::super) fn summary(hit: &SearchHit) -> Option<PullSummary> {
    let reference = PullRef { repo: repo_ref(&hit.repository.as_ref()?.name_with_owner).ok()?, number: hit.number? };
    Some(PullSummary {
        brief: PullBrief {
            reference,
            title: hit.title.clone()?,
            state: pull_state(hit.state.as_deref()?, hit.is_draft, hit.merged),
            url: hit.url.clone()?,
        },
        author: login(&hit.author),
        created_at: hit.created_at.as_deref().and_then(time::parse).unwrap_or(0),
        updated_at: hit.updated_at.as_deref().and_then(time::parse).unwrap_or(0),
        additions: hit.additions,
        deletions: hit.deletions,
        comments: hit.comments.as_ref().map_or(0, |c| c.total),
        review: decision(hit.review_decision.as_deref()),
        checks: rollup_counts(hit.commits.as_ref()),
    })
}
