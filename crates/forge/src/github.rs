//! GitHub as a [`Forge`]: GraphQL where it can, REST where GraphQL lacks the thing (a job's steps and
//! log, review requests, deleting a branch). Requests go through `gh api` run by the project, so a
//! remote project uses its host's `gh` and its sign-in. `docs/forge.md` has the mapping and the limits.
mod client;
mod gh_cli;
mod queries;
mod read;
mod briefs;
mod involved;
mod transport;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod wire;
mod write;

use std::sync::Arc;

use lathe_project::Project;
use serde_json::{Value, json};

pub use gh_cli::GhCli;
pub use transport::{Reply, Request, Transport, TransportError};

use self::{
    client::Client,
    wire::{Page, Root},
};
use crate::{
    UpdateMethod,
    ChangedFile, Check, Comment, Forge, ForgeError, ForgeResult, HeldComment, Involved, Job, JobRef, MergeOutcome,
    MergeRequest, NewLine, NewPull, Pull, PullBrief, PullRef, PullUpdate, Remark, RepoRef, Repository, Reviewer,
    Thread, ThreadId, Verdict,
};

pub struct GitHub {
    client: Client,
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
    fn with_client(client: Client) -> Self {
        Self { client }
    }

    fn vars(reference: &PullRef) -> Value {
        json!({"owner": reference.repo.owner, "name": reference.repo.name, "number": reference.number})
    }

    /// `repository.pullRequest`, or "not found" when the query answered with neither.
    fn read_pull(&self, reference: &PullRef) -> ForgeResult<(wire::Repo, wire::PullNode)> {
        let data = self.client.graphql(queries::PULL, Self::vars(reference))?.whole()?;
        let root: Root<wire::Repo> = decode(data)?;
        let mut repo = root.repository.ok_or_else(|| not_found(&reference.repo.slug()))?;
        let node = repo.pull_request.take().ok_or_else(|| not_found(&format!("{}#{}", reference.repo.slug(), reference.number)))?;
        Ok((repo, node))
    }

    /// Comments of a thread beyond the first page.
    fn rest_of_thread(&self, thread: &mut Thread, after: String) -> ForgeResult<()> {
        let more = self.client.pages(
            queries::THREAD_COMMENTS,
            json!({"thread": thread.id.0, "after": after}),
            |mut data| {
                let page: Page<wire::CommentNode> = decode(data["node"]["comments"].take())?;
                let next = page.page_info.next();
                Ok((page.nodes.into_iter().flatten().map(|n| read::comment(&n)).collect(), next))
            },
        )?;
        thread.comments.extend(more);
        Ok(())
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> ForgeResult<T> {
    serde_json::from_value(value).map_err(|e| ForgeError::Unexpected(format!("the answer has an unexpected shape: {e}")))
}

fn not_found(what: &str) -> ForgeError {
    ForgeError::NotFound(what.to_string())
}

impl Forge for GitHub {
    fn repository(&self, remote_url: &str) -> ForgeResult<Repository> {
        let repo = RepoRef::from_remote(remote_url)
            .filter(|repo| repo.host == read::HOST)
            .ok_or_else(|| ForgeError::UnknownRemote(remote_url.to_string()))?;
        let vars = json!({"owner": repo.owner, "name": repo.name});
        let root: Root<wire::Repo> = decode(self.client.graphql(queries::REPOSITORY, vars)?.whole()?)?;
        read::repository(&root.repository.ok_or_else(|| not_found(&repo.slug()))?)
    }

    fn pull(&self, reference: &PullRef) -> ForgeResult<Pull> {
        let (repo, node) = self.read_pull(reference)?;
        read::pull(&repo, &node)
    }

    fn files(&self, reference: &PullRef) -> ForgeResult<Vec<ChangedFile>> {
        self.client.pages(queries::FILES, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<wire::FileNode>>(&mut data, "files", reference)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(read::file).collect(), next))
        })
    }

    fn threads(&self, reference: &PullRef) -> ForgeResult<Vec<Thread>> {
        let pages = self.client.pages(queries::THREADS, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<wire::ThreadNode>>(&mut data, "reviewThreads", reference)?;
            let next = page.page_info.next();
            let threads = page
                .nodes
                .into_iter()
                .flatten()
                .map(|node| (read::thread(&node), node.comments.page_info.next()))
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
        self.client.pages(queries::CONVERSATION, Self::vars(reference), |mut data| {
            let page = pull_field::<Page<wire::CommentNode>>(&mut data, "comments", reference)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(read::comment).collect(), next))
        })
    }

    fn checks(&self, reference: &PullRef) -> ForgeResult<Vec<Check>> {
        self.client.pages(queries::CHECKS, Self::vars(reference), |mut data| {
            let commits: wire::Nodes<Value> = pull_field(&mut data, "commits", reference)?;
            let Some(Some(mut commit)) = commits.nodes.into_iter().last() else { return Ok((Vec::new(), None)) };
            let contexts = commit["commit"]["statusCheckRollup"]["contexts"].take();
            if contexts.is_null() {
                return Ok((Vec::new(), None));
            }
            let page: Page<wire::ContextNode> = decode(contexts)?;
            let next = page.page_info.next();
            Ok((page.nodes.iter().flatten().map(|n| read::check(&reference.repo, n)).collect(), next))
        })
    }

    fn job(&self, job: &JobRef) -> ForgeResult<Job> {
        let path = format!("repos/{}/actions/jobs/{}", job.repo.slug(), job.id);
        let reply = self.client.rest("GET", &path, None)?;
        let node: wire::RestJob = serde_json::from_str(&reply.body)
            .map_err(|e| ForgeError::Unexpected(format!("the job has an unexpected shape: {e}")))?;
        Ok(read::job(&job.repo, &node))
    }

    fn job_log(&self, job: &JobRef) -> ForgeResult<String> {
        let path = format!("repos/{}/actions/jobs/{}/logs", job.repo.slug(), job.id);
        let log = self.client.rest("GET", &path, None)?.body;
        // GitHub starts its logs with a byte order mark.
        Ok(log.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(log))
    }

    fn last_review_point(&self, reference: &PullRef) -> ForgeResult<Option<String>> {
        let (_, node) = self.read_pull(reference)?;
        Ok(read::last_review_point(&node))
    }

    fn involved(&self) -> ForgeResult<Vec<Involved>> {
        involved::involved(&self.client, None)
    }

    fn involved_in(&self, repo: &RepoRef) -> ForgeResult<Vec<Involved>> {
        involved::involved(&self.client, Some(repo))
    }

    fn briefs(&self, repo: &RepoRef, numbers: &[u64]) -> ForgeResult<Vec<Option<PullBrief>>> {
        briefs::briefs(&self.client, repo, numbers)
    }

    fn create_pull(&self, repo: &RepoRef, new: &NewPull) -> ForgeResult<PullRef> {
        self.write_create_pull(repo, new)
    }
    fn open_pull_for(&self, repo: &RepoRef, head: &str) -> ForgeResult<Option<PullBrief>> {
        briefs::open_for(&self.client, repo, head)
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

/// One field of the pull request in a query's data, taken out of it.
fn pull_field<T: serde::de::DeserializeOwned>(data: &mut Value, field: &str, reference: &PullRef) -> ForgeResult<T> {
    let pull = &mut data["repository"]["pullRequest"];
    if pull.is_null() {
        return Err(not_found(&format!("{}#{}", reference.repo.slug(), reference.number)));
    }
    decode(pull[field].take())
}

#[cfg(test)]
mod tests;
