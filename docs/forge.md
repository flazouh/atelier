# The forge

lathe reads and changes pull requests through one interface of its own, `Forge` (`crates/forge`). It
speaks GitQuiet's words (`docs/glossary.md`): a Court, a Remark, a Thread, an Unsent Comment, a Last
Review Point. Nothing above the trait names GitHub. `github` is the first implementation.

## The trait

Every call blocks and may be slow, so none runs on the UI thread. Reads a screen can wait for in parts
are separate calls: the header first, then files, threads and checks. A job's log is fetched only when a
reader opens it.

| Call | Gives |
| --- | --- |
| `repository(remote_url)` | The repository a git remote names, its merge settings, whether the reader can write. |
| `pull(ref)` | Header, review standing, check counts, what merging needs. Not files, threads or the check list. |
| `files`, `threads`, `remarks` | Changed files; review threads with all their comments; comments on the pull request as a whole. |
| `checks(ref)` | Each check on the head, with its job when it has one. |
| `job(job)`, `job_log(job)` | A job's steps and attempt; its log as text. |
| `last_review_point(ref)` | The commit the reader last reviewed up to, when the forge keeps it. |
| `involved()` | The reader's working set, each pull request on the shelf that holds it. |
| `briefs(repo, numbers)` | Many `#N` of one repository in one request. One entry per number, in order. |
| `create_pull`, `update_pull` | A new pull request; a change of title, body, base, draft or open state. |
| `merge(ref, request)` | Merge now, when ready, or into the queue. Says which it did. |
| `request_review`, `comment` | Ask people or teams; a remark on the whole pull request. |
| `hold_comment`, `held_comments`, `submit_review` | Unsent comments in the reader's review, and the verdict that sends them. |
| `reply`, `resolve` | Answer a thread; resolve or reopen it. |

Types are plain data: `Pull`, `PullSummary`, `Thread`, `Comment`, `Check`, `Job`, `MergeSettings` and
the rest in `model/`. `Involved.shelf` is `None` for a pull request the reader is only assigned to or
mentioned in. A comment held in the reader's review has `unsent: true`; the word "pending" is GitHub's and
lathe does not use it.

Errors are `ForgeError`, worded for a person: `ToolMissing`, `NotSignedIn`, `Offline`, `RateLimited`,
`NotFound`, `Denied`, `Rejected` (the forge's own reason, such as a merge with conflicts), `UnknownRemote`
and `Unexpected` (an answer lathe could not read, with what was wrong).

### What the UI gets without new types

`present` maps the model to the M0.4 models beui draws:

- `chip(&PullBrief)` gives `PrChipData`, `pr_state`, `checks`, `review_state`.
- `court_item(&Filed, now)` gives a `CourtItem`, with GitQuiet's short reason ("Ready to merge", "Checks
  running", "In the merge queue").
- `merge_facts(&Pull, checks, conflicting_files)` gives `MergeFacts`, so `beui::merge` decides the
  blockers, the button and the standing line.

### Courts

`court_of` is GitQuiet's `courtOf` from `src/domain/workingSet.ts`, with its tests case for case. The
forge says which shelf holds a pull request; the Court follows from the shelf, the state, the checks and
the review. `file_courts` files a working set: reading order, empty Courts left out, each pull request
once in its most urgent Court, the newest change first. The caller says which pull requests stand on one
that has not landed (a stack); the forge crate does not know stacks.

### The lookup for the PR chip

`Lookup::resolve(&[numbers])` answers `#N` in agent text. It asks the current repository first, all the
numbers in one request. A number that is not a pull request there is looked for among the pull requests
the reader is involved in. An answer is kept 60 seconds, including "not a pull request"; the involved list
is kept 5 minutes, since reading it is ten searches. It asks the forge only for what it does not hold.

## GitHub

### Auth and where it runs

Requests run `gh api --include --method M path [--input -]` through `Project::spawn`. A remote project
uses its host's `gh` and its sign-in, like every other process.

`gh` adds the token itself. It never passes through lathe, so lathe has no token to store, log or print.
This differs from reading `gh auth token`, on purpose: the best place for a secret is a place lathe never
sees it. `--include` puts the status line and headers before the body, so a failure keeps its body and a
rate limit shows its reset.

| What `gh` does | lathe says |
| --- | --- |
| Not installed on the host | `ToolMissing { tool: "gh" }` |
| A 401 (the token no longer works), or exit 4, or stderr with `gh auth login` or `Bad credentials` | `NotSignedIn` |
| No reply and stderr with `connection refused`, `no such host`, `check your internet connection`, `i/o timeout`, `network is unreachable` or `tls handshake` | `Offline` |
| No reply and any other stderr | `Unexpected`, with the last line `gh` wrote |

`Project::spawn` keeps the last 64 KB of the process's stderr (`Control::stderr()`), and lathe reads `gh`'s
own words from it. The cases are tests on stderr captured from `gh` 2.101 (`tests/fixtures/gh_stderr/`).
`Control::wait` returns with the stderr complete.

### What is GraphQL and what is REST

GraphQL, in `github/queries/*.graphql`: the repository and its settings, the pull request header, files,
threads, remarks, checks, the involved searches, the batched lookup, and every change except three.
REST: a job's steps (`GET actions/jobs/{id}`), a job's log (`GET actions/jobs/{id}/logs`), review requests
(`POST pulls/{n}/requested_reviewers`), and deleting the head branch after a merge
(`DELETE git/refs/heads/{name}`). GraphQL has no job steps and no log, and REST is simpler for the rest.

A GitHub Actions job has the id of its check run, so a check that belongs to a workflow run carries a
`JobRef` and its steps and log can be fetched. A check from another app has none.

### Shelves

GitHub's own dashboard sorts by an internal category that its public API does not give. `involved()`
rebuilds the shelves from ten searches, all `is:pr is:open archived:false` and run at once:

| Shelf | Search |
| --- | --- |
| Needs action | `user-review-requested:@me`; `author:@me review:changes_requested`; `author:@me draft:false status:failure` |
| Team review requested | `team-review-requested:@me` |
| Waiting for review | `author:@me draft:false -review:approved` |
| Ready to merge | `author:@me draft:false review:approved` |
| Your drafts | `author:@me draft:true` |
| Merge queue | `author:@me is:queued` |
| none | `assignee:@me`; `mentions:@me` |

A pull request found by a search of no shelf and by a shelf is one row, on the shelf. These searches
approximate GitHub's categories; they are not the same rule. Open item: check them against GitHub's own
list on a busy account.

### Rate limits and retries

- A 429, a 403 with `retry-after` or `x-ratelimit-remaining: 0`, and a GraphQL error of type
  `RATE_LIMITED` (which comes with a 200) all mean "slow down".
- The wait is `retry-after`, else the time to `x-ratelimit-reset`. Up to 60 seconds, the client waits and
  tries again, up to 4 tries in all. A longer wait returns `RateLimited { retry_after }` at once, so the
  UI can say so, and the caller schedules the retry.
- A server error (5xx) on a read is tried again with pauses of 1, 2 and 4 seconds.
- A change is never sent twice: a mutation, or a REST POST or DELETE, that meets a server error is not
  retried, because it may have landed. A rate limit is retried for a change too: GitHub refuses those
  before it acts.
- Pagination follows the cursor, 100 items a page (50 threads), up to 500 pages, then errors rather than
  hold a call for ever. A thread with more comments than its first page fetches the rest by its id.

### Mapping notes

- A merged pull request has `merge_state: Unknown`: GitHub does not compute it after a merge.
- `Check.required` comes from GitHub's `isRequired` on each check. `Pull.checks` counts every check;
  `present::merge_facts` counts only the required ones when it is given the check list, and otherwise only
  when GitHub says a rule blocks the merge.
- GitHub says a branch conflicts, not which files. `merge_facts` takes the files the reader found (a
  local `git merge-tree` through `Project::git`) and puts the base branch in their place when it has none.
  Open item: the app must run the merge-tree.
- An outdated thread has no current line; `original_line` says where it was written.
- A GitHub log starts with a byte order mark; `job_log` removes it.
- Merge queue: a repository that has one takes `merge` into the queue (`enqueuePullRequest`), whatever
  `when_ready` says. `delete_branch` applies to a merge that lands now, in the same repository; a branch
  in a fork is never deleted.
- `Pull.base_sha` is GitHub's `baseRefOid`: the tip of the base branch the pull request was last compared
  with. The pull request view takes the merge base of it and the head as the start of the diff
  (`docs/pr-view.md`). Every type in `model/` is `Serialize` and `Deserialize`, for the view's snapshots.
- `time::ago` and `time::parse` turn the forge's timestamps into words and seconds.

## Tests

- Unit tests hold the retry, rate limit, pagination, `gh` output parsing, time and URL rules.
- `tests/replay.rs` replays answers recorded from GitHub for `oven-sh/bun#44169` (a public merged pull
  request with five threads, five remarks, thirteen checks). Recorded with `tests/live.rs`:
  `LATHE_FORGE_RECORD=crates/forge/tests/fixtures/github cargo test -p lathe-forge --test live -- --ignored`.
  Recording refuses any request that is not a query or a GET. The recorded answers are bent to reach the
  cases the one pull request lacks: a draft, a conflict, a queue, several pages.
- The working set is tested on hand-made answers, since a recording would hold the private pull requests of
  whoever recorded.
- Every write is tested on hand-made answers in GitHub's documented shapes, and checks the request it
  sent. No test creates, comments, reviews, merges or pushes anything on GitHub.
- `tests/schema.rs` (ignored, read-only) runs every write against answers held in memory, and checks the
  input each one built against the fields GitHub's schema gives for its input type. 19 mutations checked,
  none sent. It fails before the first live write if GitHub renamed or dropped a field.
- `tests/live.rs` (ignored, read-only) reads a public pull request end to end through the real `gh`.
- The first live write test waits for Alex to approve a scratch repository (M4).

## Open

- Live writes are untested. The shape of every input is checked against GitHub's schema; what GitHub does
  with them is not.
- The working set takes about 5 seconds live (ten searches at once, 54 pull requests, HP under load). Fewer
  fields per row, or a lower cap on pages, would cut it. See `docs/performance.md`.
- The shelves are searches, not GitHub's own categories.
- Conflicting files are not in GitHub's answer.
- Only `github.com`. A GitHub Enterprise host needs `gh api --hostname` and a host in `RepoRef`.
- Stacks: nothing here knows them. `file_courts` takes a function that says which pull requests stand on
  an unlanded one.
