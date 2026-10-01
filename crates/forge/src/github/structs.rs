use std::sync::Arc;

use atelier_project::Project;
use serde_json::{Value, json};

pub use super::gh_cli::GhCli;
pub use super::transport::Transport;
use super::{
    client::Client,
    wire::{Page, Root},
};
use crate::{
    ChangedFile, Check, Comment, Forge, ForgeError, ForgeResult, HeldComment, Involved, Job,
    JobRef, MergeOutcome, MergeRequest, NewLine, NewPull, Pull, PullBrief, PullRef, PullUpdate,
    Remark, RepoRef, Repository, Reviewer, Thread, ThreadId, UpdateMethod, Verdict,
};
use super::helpers::{decode, not_found, pull_field};

pub struct GitHub {
    pub(super) client: Client,
}

impl GitHub {
    /// GitHub through the `gh` the project's host has.
    pub fn new(project: Arc<dyn Project>) -> Self {
        Self::with_transport(GhCli::new(project))
    }

    pub fn with_transport(transport: impl Transport + 'static) -> Self {
        Self { client: Client::new(transport) }
    }

    #[cfg(test)]
    pub(super) fn with_client(client: Client) -> Self {
        Self { client }
    }

    pub(super) fn vars(reference: &PullRef) -> Value {
        json!({"owner": reference.repo.owner, "name": reference.repo.name, "number": reference.number})
    }

    /// `repository.pullRequest`, or "not found" when the query answered with neither.
    pub(super) fn read_pull(&self, reference: &PullRef) -> ForgeResult<(super::wire::Repo, super::wire::PullNode)> {
        let data = self.client.graphql(super::queries::PULL, Self::vars(reference))?.whole()?;
        let root: Root<super::wire::Repo> = decode(data)?;
        let mut repo = root.repository.ok_or_else(|| not_found(&reference.repo.slug()))?;
        let node = repo.pull_request.take().ok_or_else(|| not_found(&format!("{}#{}", reference.repo.slug(), reference.number)))?;
        Ok((repo, node))
    }

    /// Comments of a thread beyond the first page.
    pub(super) fn rest_of_thread(&self, thread: &mut Thread, after: String) -> ForgeResult<()> {
        let more = self.client.pages(
            super::queries::THREAD_COMMENTS,
            json!({"thread": thread.id.0, "after": after}),
            |mut data| {
                let page: Page<super::wire::CommentNode> = decode(data["node"]["comments"].take())?;
                let next = page.page_info.next();
                Ok((page.nodes.into_iter().flatten().map(|n| super::read::comment(&n)).collect(), next))
            },
        )?;
        thread.comments.extend(more);
        Ok(())
    }
}

impl Forge for GitHub {
    fn repository(&self, remote_url: &str) -> ForgeResult<Repository> {
        let repo = RepoRef::from_remote(remote_url)
            .filter(|repo| repo.host == super::read::HOST)
            .ok_or_else(|| ForgeError::UnknownRemote(remote_url.to_string()))?;
        let vars = json!({"owner": repo.owner, "name": repo.name});
        let root: Root<super::wire::Repo> = decode(self.client.graphql(super::queries::REPOSITORY, vars)?.whole()?)?;
        super::read::repository(&root.repository.ok_or_else(|| not_found(&repo.slug()))?)
    }

    fn pull(&self, reference: &PullRef) -> ForgeResult<Pull> {
        let (repo, node) = self.read_pull(reference)?;
        super::read::pull(&repo, &node)
    }

    fn files(&self, reference: &PullRef) -> ForgeResult<Vec<ChangedFile>> {
        self.client.pages(super::queries::FILES, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<super::wire::FileNode>>(&mut data, "files", reference)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(super::read::file).collect(), next))
        })
    }

    fn threads(&self, reference: &PullRef) -> ForgeResult<Vec<Thread>> {
        let pages = self.client.pages(super::queries::THREADS, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<super::wire::ThreadNode>>(&mut data, "reviewThreads", reference)?;
            let next = page.page_info.next();
            let threads = page
                .nodes
                .into_iter()
                .flatten()
                .map(|node| (super::read::thread(&node), node.comments.page_info.next()))
                .collect();
            Ok((threads, next))
        })?;
        pages
            .into_iter()
            .map(|(mut thread, more)| {
                if let Some(after) = more {
                    self.rest_of_thread(&mut thread, after)?;
                }
                Ok(thread)
            })
            .collect()
    }

    fn remarks(&self, reference: &PullRef) -> ForgeResult<Vec<Remark>> {
        self.client.pages(super::queries::CONVERSATION, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<super::wire::CommentNode>>(&mut data, "comments", reference)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(super::read::comment).collect(), next))
        })
    }

    fn checks(&self, reference: &PullRef) -> ForgeResult<Vec<Check>> {
        self.client.pages(super::queries::CHECKS, Self::vars(reference), |mut data| {
            let commits: super::wire::Nodes<Value> = pull_field(&mut data, "commits", reference)?;
            let Some(Some(mut commit)) = commits.nodes.into_iter().last() else { return Ok((Vec::new(), None)) };
            let contexts = commit["commit"]["statusCheckRollup"]["contexts"].take();
            if contexts.is_null() {
                return Ok((Vec::new(), None));
            }
            let page: Page<super::wire::ContextNode> = decode(contexts)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(|n| super::read::check(&reference.repo, n)).collect(), next))
        })
    }

    fn job(&self, job: &JobRef) -> ForgeResult<Job> {
        let path = format!("repos/{}/actions/jobs/{}", job.repo.slug(), job.id);
        let reply = self.client.rest("GET", &path, None)?;
        let node: super::wire::RestJob = serde_json::from_str(&reply.body)
            .map_err(|e| ForgeError::Unexpected(format!("the job has an unexpected shape: {e}")))?;
        Ok(super::read::job(&job.repo, &node))
    }

    fn job_log(&self, job: &JobRef) -> ForgeResult<String> {
        let path = format!("repos/{}/actions/jobs/{}/logs", job.repo.slug(), job.id);
        let log = self.client.rest("GET", &path, None)?.body;
        // GitHub starts its logs with a byte order mark.
        Ok(log.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(log))
    }

    fn last_review_point(&self, reference: &PullRef) -> ForgeResult<Option<String>> {
        let (_, node) = self.read_pull(reference)?;
        Ok(super::read::last_review_point(&node))
    }

    fn involved(&self) -> ForgeResult<Vec<Involved>> {
        super::involved::involved(&self.client, None)
    }

    fn involved_in(&self, repo: &RepoRef) -> ForgeResult<Vec<Involved>> {
        super::involved::involved(&self.client, Some(repo))
    }

    fn briefs(&self, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
        super::briefs::briefs(&self.client, repo, numbers)
    }

    fn create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef> {
        self.write_create_pull(repo, new)
    }
    fn open_pull_for(&self, repo: &RepoRef, head: &str) -> ForgeResult<Option<PullBrief>> {
        super::briefs::open_for(&self.client, repo, head)
    }

    fn update_pull(&self, reference: &PullRef, update: &PullUpdate) -> ForgeResult<()> {
        self.write_update_pull(reference, update)
    }

    fn merge(&self, reference: &PullRef, request: &MergeRequest) -> ForgeResult<MergeOutcome> {
        self.write_merge(reference, request)
    }

    fn update_branch(&self, reference: &PullRef, method: UpdateMethod, expected_head: &str) -> ForgeResult<()> {
        self.write_update_branch(reference, method, expected_head)
    }

    fn cancel_auto_merge(&self, reference: &PullRef) -> ForgeResult<()> {
        self.write_cancel_auto_merge(reference)
    }

    fn dequeue(&self, reference: &PullRef) -> ForgeResult<()> {
        self.write_dequeue(reference)
    }

    fn delete_branch(&self, reference: &PullRef) -> ForgeResult<()> {
        self.write_delete_branch(reference)
    }

    fn revert(&self, reference: &PullRef) -> ForgeResult<PullRef> {
        self.write_revert(reference)
    }

    fn request_review(&self, reference: &PullRef, reviewers: &[Reviewer]) -> ForgeResult<()> {
        self.write_request_review(reference, reviewers)
    }

    fn comment(&self, reference: &PullRef, body: &str) -> ForgeResult<Comment> {
        self.write_comment(reference, body)
    }

    fn hold_comment(&self, reference: &PullRef, comment: &NewLine) -> ForgeResult<HeldComment> {
        self.write_hold_comment(reference, comment)
    }

    fn held_comments(&self, reference: &PullRef) -> ForgeResult<Vec<HeldComment>> {
        Ok(self
            .threads(reference)?
            .into_iter()
            .flat_map(|thread| {
                let (id, path, line) = (thread.id, thread.path, thread.line.or(thread.original_line));
                thread.comments.into_iter().filter(|c| c.unsent).map(move |comment| HeldComment {
                    thread: id.clone(),
                    comment,
                    path: path.clone(),
                    line,
                })
            })
            .collect())
    }

    fn submit_review(&self, reference: &PullRef, verdict: Verdict, body: &str) -> ForgeResult<()> {
        self.write_submit_review(reference, verdict, body)
    }

    fn reply(&self, thread: &ThreadId, body: &str) -> ForgeResult<Comment> {
        self.write_reply(thread, body)
    }

    fn resolve(&self, thread: &ThreadId, resolved: bool) -> ForgeResult<()> {
        self.write_resolve(thread, resolved)
    }
}
