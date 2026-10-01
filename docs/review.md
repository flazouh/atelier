# Reviewing what the agent did

`crates/review` turns a turn of an agent into something a reader can review: the files it changed, the
hunks in each, and a way to send comments back. It is pure text and git. It has no window and no agent
names, and it reads the project only through `Project`, so it works on a remote project as on a local one.

It is a crate of its own, not a module of `crates/agents`, because it reads git and diffs files and
depends on atelier-ui's `InlineHunk`. `crates/agents` stays the session model and its backends.

## The baseline

A turn's baseline of a file is the file's text at the moment the agent first touched it in that turn.
Two things find a touched file.

1. **A tool call that names a file.** The tracker takes the file's text when the neutral events say an
   `Edit` or `Write` call is about to change it. It looks at these events, earliest first:
   - `ToolTarget { id, file }`: the stream names the file before the call's input is whole, so before
     the tool runs. `crates/agents` reads the file out of the streaming JSON (`input_json_delta`).
   - `Permission`: the question carries the call and its file.
   - `ToolStarted` with a file, and `ToolInput` with a file, for backends that give the file later.
   Only the first touch counts. A file that did not exist has the baseline "absent". A file that is not
   text has a hash of its bytes, so a rewrite with the same bytes is no change.
2. **Git.** A shell command changes files no tool names. At the start of a turn the tracker takes
   `git status --porcelain=v2 -z --untracked-files=all` and a blob id (`git hash-object`) for each file that
   differs from the last commit. At the end it takes both again. A file whose entry or blob id differs is a
   file the turn changed. Paths are made relative to the project, so a project in a folder of a larger
   repository sees only its own files.

How exact the baseline of a file git found is:

| The file when the turn started | Baseline | Exact |
| --- | --- | --- |
| Named by a tool call | Its text at the first touch | yes |
| Clean (equal to the last commit) | The last commit's text | yes |
| Not in the last commit and not there at the start | Absent (a new file) | yes |
| Changed by the user, not committed | The last commit's text, which also holds the user's edits as if the agent made them | no |
| Untracked, then changed by a command | Unknown: listed, no hunks | no |
| Changed by the user, then put back to the last commit by a command | Unknown: listed, no hunks (the user's text is lost, and the reader should know) | no |

The last commit's text of many files comes from one `git cat-file --batch` process (a turn can change
hundreds of files).

`TurnTracker::begin(project)`, `observe(project, &event)` for each event of the session, and
`finish(project) -> TurnReview`. All three read the project and may be slow: call them off the UI thread.

### Later turns

Every turn has a tracker of its own, and it takes its baselines from the disk at the first touch. So a
later turn that edits the same file diffs against the text the user kept, not against the first baseline:
what the user rejected is back in the file, and what the user accepted is in it.

`SessionReview` holds the turns. `turns()[i]` is one turn. `whole()` is the session as one change: each
file against its text before the first turn that touched it and as the last turn left it. A file a later
turn put back is not in it.

## The turn's end

For each file: `FileReview { path, change, content, before, after, exact }`.

- `change`: `Modified`, `Added`, `Deleted`, or `Renamed { from }`. A file gone and a file new with the same
  text are one rename. A moved file that was edited too is a delete and an add.
- `content`: `Text(Merged)`, `Binary` (listed, no hunks) or `Unknown`.
- `version()`: changes when the text now changes. `Reviewed` marks a file read in a turn against it, so the
  mark expires when the file changes, as GitQuiet's Reviewed State does.

`present::changed_files(&files)` gives the `Vec<ChangedFile>` (`+a -r`, added, deleted, renamed) that
`ChangedFiles` and `ChangedFileTree` take. `present::hunks(&file)` gives the `InlineHunk`s.

## Merged: one file under review

`Merged` is the file in the form the inline review edits: one text, each changed hunk as its old rows and
then its new rows. `InlineHunk { id, removed, added }` names the rows, with `removed.end == added.start`.

- `Merged::diff(baseline, current)` builds it.
- `text()`, `hunks()`, `counts()`, `baseline()`, `current()`. Both texts come back exactly, line ends
  included: the final line end is kept apart, so a hunk at the end of a file that has none does not run
  into the next row.
- `decide(id, Accept | Reject)`: the closing rows go, the survivors stay as plain code, the other hunks
  move up. The text is what `inline_review::apply_to_text` gives. After it, `baseline()` holds what the user
  accepted and `current()` no longer holds what the user rejected.
- `edited(after)`: the user typed in the merged buffer. The hunks move with `inline_review::track_edit`.
- `rebased_on(current)`: the agent wrote the file again while it is under review. It diffs the baseline as
  it stands, with what the user accepted, against the new text. What the user decided stays decided, and a
  hunk that did not change keeps its id.
- `anchor(rows)`: the lines a comment on merged rows names, in the version the rows belong to.

Hunk ids are a hash of the hunk's rows, and a number for hunks that say the same thing.

How the inline review uses it: `Merged::text()` goes in the editor buffer; the hunks are the bands and
bars. A decision calls `decide`. The user's typing calls `edited`. When the agent's edit lands on the file
(the session's `ToolFinished` for an `Edit` or `Write`, or a change on disk), the app reads the file and calls
`rebased_on`, then shows the new hunks with the old decisions kept. A decision that rejects a hunk must also
write the file, since `current()` no longer holds the rejected rows: the app does that write.

## The diff

`imara-diff` 0.2, the Histogram algorithm, then its indentation-based slider cleanup.

- **Why not `similar`:** `similar` has a friendlier API and character-level output built in, but imara's
  Histogram is faster on large files (its own benchmarks say up to 30 times faster than `similar` on
  Linux kernel versions), and the targets here are on 20,000-line files. imara diffs any token sequence, so the same code
  does lines and words.
- **Lines:** rows without their line ends are the tokens, so a change of a final line end alone is not a
  hunk (the flags say it).
- **Words:** `word_changes(old, new)` diffs two rows in words (a run of letters, digits and underscores; a
  run of spaces; any other character alone) and gives the byte ranges that differ. `pair_rows(removed,
  added)` pairs the rows of a hunk that are the same line changed (at least 40% of their bytes the same), so the review can
  highlight words in them and leave the rows that were written or removed whole.

## Comments

A comment is a row range with its quoted rows: `Comments::add(turn, path, anchor, body)` where the
anchor comes from `Merged::anchor`. It holds the lines and the quote. When the file changes,
`reanchor(path, current_text)` follows the quote to the nearest place it appears now; a comment whose rows
are gone is `stale` and keeps its old lines. A comment on removed rows counts lines in the text before the
turn, which does not change.

`take_attachments()` gives the comments as `Attachment::LineComment { path, first_line, last_line,
removed, quote, body }` and clears them. They go in the next message:

```rust
session.send(Command::Send { text, attachments })
```

`Attachment` is part of the neutral session model (`crates/agents`), and `message_text` writes a message
and its attachments as text for a backend that takes only text (Claude Code does):

```
Please fix these

Review comment on src/a.rs, lines 10-12:
> let a = 1;
> let b = 2;
Why not a struct?
```

## Performance

See `docs/performance.md`, "Review". A turn of 200 files, one of them 20,000 lines, is 55 ms from
`finish` on real files with git; the hunks alone are 20 ms. One edit in a 20,000-line file is diffed in
1.7 ms.

## Limits

- **The baseline is taken when the file is named.** `ToolTarget` comes while the input still streams, so
  before the tool runs. A backend that names the file only at execution can lose the race, most likely with
  a slow remote project. Git catches such a file at the end, with an exact baseline if the file was clean.
- **The user's own edits during a turn** look like the agent's, since the baseline is the disk at the
  first touch and the end is the disk at the end.
- **Git sees tracked files, and untracked files not ignored.** A command that changes an ignored file, in a
  project with no tool naming it, is not seen.
- **A project that is not a git repository** has only the files tool calls named.
- **A rename with an edit** is a delete and an add. Only an exact same-text move is a rename.
- **A file mode change, a symlink,** and a file too large to read are not handled. A file that is not valid
  UTF-8, or has a NUL byte, is binary.
- **Line ends:** rows keep a carriage return, so a file whose line ends changed shows every row changed.
- A turn that was interrupted still ends its review with what changed.
- Not built: the UI, the writes a decision needs, and the turn boundaries (the app decides when a turn
  starts and ends, from `TurnEnded` and the user's message).
