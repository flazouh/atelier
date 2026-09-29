# The pull request view

`crates/pr-view` (`lathe-pr-view`) is the pull request view on real data: the list of pull requests, and
one pull request with its description, checks, conversation, verdict, merge box, commits, tree and diff.
The parts are beui's (`docs/review.md`, `docs/inline-review.md`). The data is the forge's (`docs/forge.md`)
and git's. This crate joins them.

## The app's side

    let hub = cx.new(|cx| PrHub::new(project, forge, config, cx))?;   // small local files, opened at once
    hub.update(cx, |hub, cx| hub.open(pull_ref, window, cx));         // one pull request; the list is `hub.list()`
    cx.subscribe(&hub, |_, _, event: &PrEvent, cx| match event { .. });

| Name | What it is |
| --- | --- |
| `PrHub` | The list and at most one open pull request. Render it. `open` needs the window (it makes the view's editor). |
| `PrConfig` | `me`, `read_only`, `remote_data`, `local_data`, `workers`, `refresh`, `list_refresh`, `fetch_url`. |
| `PrEvent::OpenFile { pull, path, line }` | The reader asked to open a file in the editor. |
| `PrEvent::OpenSession(pull)` | The reader pressed the linked session. |
| `PrEvent::Closed(pull)` | The reader went back to the list. |
| `hub.set_linked_session`, `hub.forget_checkout`, `hub.close` | The app links a session, drops a checkout, goes back. |

`read_only(true)` sends nothing: every write says "Read-only: nothing was sent." The app supplies
`Workers` (`lathe_lsp`) if it wants language servers; without them the diff has no lookups.

## Where the bytes come from

Never the reader's checkout. Each forge repository gets a bare cache, `git clone --bare --shared` from the
project, in the data folder (`remote_data` on a remote project, so git runs where the code is). The cache
fetches `refs/pull/N/head` into `refs/lathe/pr/N/head` and the base branch into `refs/lathe/base/<branch>`.
The base of the diff is the merge base of the forge's `baseRefOid` and the head. Files come from
`git diff`, blobs from one `git cat-file --batch`. Every call goes through `Project::spawn` with an
argument list (no shell text), `GIT_TERMINAL_PROMPT=0` and `LC_ALL=C`, so SSH projects work the same way.
There is no `git worktree`.

A language server needs real files. The head checkout is `git archive | tar -x` into the data folder, with
a sidecar `head-N.sha`; a push replaces it. It goes when the pull request merges or closes (the hub sweeps
the list for closed ones, and `forget_checkout` removes one at once).

## What the reader sees

- **Showing**: the whole pull request, since the last review (the forge's Last Review Point, when the head
  has moved past it), or since any commit. If the review point is gone from history, the view shows the
  whole pull request and says so.
- **Reviewed State** is per file version. `x` marks the file on screen; the mark stores the new blob's id
  in SQLite (`pr-view/reviewed.sqlite` in `local_data`, migrations by `user_version`). A push that leaves
  a file's bytes alone keeps the mark; a push that changes them clears it. `Put back` clears all.
- **Threads** sit under their line in the diff. A thread on code that changed since, or on the whole file,
  is counted above the diff. A thread with more than three comments shows the first and the last, and a
  line to show the rest.
- **Conversation** in the rail lists 20 threads and 20 remarks at first and counts all of them; "Show
  more" adds 20.
- **Unsent comments**: outside a review pass a line comment is sent at once. Once one is held, comments are
  held too until the reader sends the review. "Start a review" holds the first.
- **Brought In**: `t` (Go to file) opens any file of the head beside the changed ones.

## Live updates

A background loop asks the forge for the header on a cadence (`refresh`, slower when nothing changes, up to
four times; doubled after a failure up to 300 s; the forge's own wait after a rate limit, up to an hour; it
stops for `NotSignedIn`, `ToolMissing`, `UnknownRemote`, `Denied` and `NotFound`, and the view offers
"Try again"). It reads the other parts only when the header changed. A change is told in one line ("1 new
comment, checks changed") that stays until something else replaces it. The reader keeps their file, and
the same line of it where it still exists.

## What is stored

| File | Holds |
| --- | --- |
| `pr-view/snapshots/github.com-owner-name-N.json` | The last read of a pull request. The next open draws from it at once. |
| `pr-view/involved.json` | The last list. |
| `pr-view/reviewed.sqlite` | Reviewed State and when each pull request was opened (for "unread"). |

Snapshots are a versioned envelope, written atomically; a damaged file is a miss, not an error.

## Fixtures and the story

`fixture/` holds an in-memory forge (`FixtureForge`, with a log of writes and failures on demand), a real
git repository made on disk (`Repo`), the story's "relay" pull request (`Relay`), a large one for numbers
(`Big`) and `Real`, which reads a real pull request through `gh` (`PRV_REAL=owner/name#number`, read only).
The gallery story "Pull request view" mounts the hub on one of them. `PRV_VIEW=list` opens the list;
`PRV_LSP=1` starts the language servers; `PRV_BIG=1` and `GALLERY_SCROLL=1` are the numbers
(`docs/performance.md`).

## Not built

- **Tolerated checks** (the reader's list of checks that may fail) are not kept.
- The merge box's Ready for review, Update branch, Cancel, Remove from queue, Delete branch and Revert say
  "not built yet": the `Forge` trait has no call for them.
- Writes are tested against the fixture forge only. No test has written to GitHub.
- Uses and Names lookups in the diff.
