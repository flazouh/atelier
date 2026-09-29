# The lathe app

`lathe` is the app; the gallery stays the design system's workshop. This file describes the shell, the
Project interface every part of the app reads and writes through, and (from M1b) the remote protocol.

## Crates

| Crate | Binary | What it holds |
| --- | --- | --- |
| `crates/project` (`lathe-project`) | | The `Project` interface and `LocalProject`. No GPUI. |
| `crates/settings` (`lathe-settings`) | | The settings file: the theme and the recent projects. |
| `crates/editor` (`lathe-editor`) | | A file's editor wired to its language server (moved from the gallery). |
| `crates/app` (`lathe-app`) | `lathe` | The window, the shell and the project state. |
| `crates/remote` (`lathe-remote`) | `lathe-remote` | M1b: the Project interface served over stdio. |

`lathe-lsp` starts its servers through the Project's `spawn`, so a server runs where the project is.

## The Project interface

One trait, blocking, and never called on the UI thread: the app calls it from background tasks and
hands the answers to the UI. A local project answers from the disk; a remote one (M1b) sends each call
over the SSH pipe and waits for the answer, with a timeout. Nothing above the trait knows which.

| Call | What it does |
| --- | --- |
| `root()` | The project's folder, as the host names it. |
| `list()` | Every file and folder under the root, `.gitignore` respected, `.git` left out, sorted by path. |
| `read(path)` | A file's bytes. |
| `write(path, bytes)` | Writes a file whole: a temporary file beside it, then a rename. |
| `watch(sink)` | Reports created, changed and removed paths in batches, until the handle drops. |
| `search(query)` | Lines that match a literal or a regex, `.gitignore` respected, with a cap. |
| `spawn(command)` | A process with piped stdin and stdout, and a handle to kill it, wait for it, and read the last 64KB of its stderr. |
| `git(args)` | Runs `git` in the root and returns its status and output. |

Paths are relative to the root in the interface, except `spawn`'s working folder and a language
server's own paths, which are the host's absolute paths. A path that climbs out of the root (`..`),
or is absolute, is refused.

A watch batches what happens in 50 ms. New and removed paths list the tree again; a changed open
file reloads when its tab is clean. The app's tests fire a project's watch themselves, on the test
thread (`Quiet` in `crates/app/src/open_project/tests.rs`), because GPUI's test scheduler rejects a
wake from the watcher's own thread; lathe-project tests the real watcher.

## The shell

- The window draws its own title bar area. On macOS the traffic lights sit inset in the page, as in
  Zed and Cursor. On Linux the layout is the same, without them.
- Left: the sidebar, with the projects open in this window and each project's sessions (empty until
  M2), then its file tree.
- Middle: the agent panel. Right: the editor tabs, or the review.
- The splits are gpui-base's resizable panels, with a handle that shows only as a wash on hover.
- Foot: the status line: the project, its branch, and the language server's state.
- Keys: GitQuiet's table where a command applies (⌘B the left pane, ⌘⇧B the right pane), ⌘O open
  folder, ⌘S save, ⌘W close tab, ⌘J the bottom panel once there is one. The side panes keep their
  width when one hides; the agent panel takes what is left.
- `t` (GitQuiet's Go to file, while nothing is being typed) opens beui's Finder over the project's
  files; Enter opens the one picked, Escape closes it.
- `lathe [folder…]` opens each folder named as a project.

## Projects

- Open Folder (⌘O) uses the native folder picker. The start screen and the sidebar list recent
  projects. The settings file keeps them across launches.
- One window holds several projects. Switching keeps each project's tree, tabs and servers, so it is
  instant.
- The file tree is listed and watched off the UI thread, and drawn as a virtual list.
- A file opens in an editor tab with its language server: one `Workers` pool per project root. A tab
  shows a dot while dirty. A file that changes on disk reloads when its tab is clean. A dirty tab
  keeps its edits and says so, with Reload and Keep mine.
- Go to definition (F12, or ⌘/ctrl and a click) into another project file opens it in a tab at the
  place; one outside the project, such as the toolchain's sources, is named in the status line.
  gpui-base patch 17 makes the new tab scroll to that place.
- Closing a tab (⌘W or its button) with unsaved edits asks: Save, Don't Save, or Cancel. Closing the
  window with unsaved edits asks too.
- Empty states: no project (the start screen, with Open Folder and the recent list), an empty folder,
  and a folder with no git.

## QA, M1a

Driven on the HP under Xvfb with `tools/app-drive-linux.sh`, in a throwaway clone of the repository
(`/tmp/qa-lathe`, its `target` linked to the checkout's, so rust-analyzer loads quickly). The
recordings and stills are in `~/shots/m1/` on the HP.

| What | Seen |
| --- | --- |
| Open a local folder, browse the tree | `qa-1-open.png`: 1008 files, listed in 10 to 57 ms |
| Open a Rust file | rust-analyzer ready; "nothing wrong" |
| Hover | `qa-2-hover.png`: `std::time::Instant`'s card |
| Go to definition | `qa-3-underline.png`, `qa-3-definition.png`: `bind_keys` opens `shell.rs` at line 39 |
| Edit and save | `qa-4-dirty.png` (the dot), `qa-5-saved.png` ("Saved crates/app/src/shell.rs"); `git diff` in the clone shows the line |
| Several projects, switching | `multi-*.png`: three projects; an empty folder and one with no git say so |
| ⌘B, ⌘⇧B | `panes-*.png` |
| Go to file | `goto-1.png` ("openpro" finds `open_project.rs`), `goto.png` (Enter opens it) |
| Changed on disk | `disk-*.png`: a clean tab reloads; a dirty one asks; Keep mine and Reload |
| Recording | `qa-local.mp4`, `multi.mp4`, `disk.mp4` |

## Remote projects (M1b)

A project on an SSH host is a `RemoteProject` (`crates/remote`): every `Project` call becomes a
request to `lathe-remote`, which runs on the host and answers with a `LocalProject` there. Nothing
above the interface knows the difference: the tree, the editor, search, git and the language
servers all go through the same calls. The servers run on the host, started through the remote
`spawn`, found by the host's `PATH` (`Store::on_host`), each at the project's root
(`Workers::at_project_root`); lathe downloads nothing onto a host.

### The protocol

- Frames: a four-byte little-endian length, then postcard bytes (`crates/remote/src/protocol.rs`).
  postcard over JSON because a file crosses as raw bytes (no base64), frames stay small, and the
  frames are the interface's own serde types. A frame over 256 MB is refused.
- The app sends `Request { id, call }`; the host answers `Response { id, result }` and on its own
  sends `Event`s: a watch's `Changes`, a process's `Output`, its `Exited`. The first call is
  `Hello { version, root }`; a version the host does not speak fails the hello.
- The host runs each request on a thread of its own, so a slow search never holds up a read. A
  process's stdin is fed in order by a thread of its own. Its stderr's last 64 KB is kept.
- Every call has a timeout: 60 s for a listing, a search or git, 30 s for the rest. A timeout is an
  error that names the host ("hp-agent did not answer in 30 s").

### Connecting

`ssh -o BatchMode=yes -o ConnectTimeout=10 -o ServerAliveInterval=15 -o ServerAliveCountMax=3`, the
user's own ssh with their config, keys and agent; `BatchMode` makes a host that wants a password
fail at once, in ssh's words, instead of waiting on a prompt nobody sees.

1. Probe: `uname -sm` and `$HOME`.
2. Deploy: `~/.cache/lathe/remote/<version>-<hash>/lathe-remote`, where the hash is of the copy the
   app would upload, must answer `--version`. If not, the copy goes up over ssh: `cat` into a
   `.part` file, `chmod +x`, and a rename, so a half copy never runs. This needs no `scp`.
3. Dial: `ssh <host> <that path> --stdio`. On the host, lathe-remote takes its `PATH` from the
   user's login shell, and adds `~/.cargo/bin` and `~/.local/bin` if missing, so it finds the
   language servers the user installed.

The copy to upload comes from `$LATHE_REMOTE_DIR/<system>-<arch>/lathe-remote` (`linux-x86_64`,
`darwin-aarch64`), or, for a host like this machine, the `lathe-remote` beside the app. In
development the HP builds the linux-x86_64 copy (`cargo build --release -p lathe-remote`); a Mac
app that opens a project on the HP points `LATHE_REMOTE_DIR` at a folder holding that file as
`linux-x86_64/lathe-remote`. `tools/build-remote.sh` builds it and prints that folder; it sets its
own PATH, so a plain `ssh hp-agent ~/code/local/lathe/tools/build-remote.sh` works.

### Failures

| Failure | What the app does |
| --- | --- |
| Host unreachable | The form shows ssh's words: "ssh: Could not resolve hostname …". |
| Auth fails | The form shows "Permission denied (publickey)." |
| The connection drops, or lathe-remote crashes | Every waiting call fails at once. A banner says "Lost hp-agent:/path (why). Reconnecting; your unsaved edits are kept here." The client dials again, backing off from 0.5 s to 30 s; back, it says hello, restores the watch, lists the tree, and starts each open file's language server again. Tabs keep their text and dirty marks, and a save after the reconnect writes them. |
| A slow link | A file being read shows as a pending tab with a spinner; a call that runs out of time says so in the status line. |

Opening: the start screen's "Open over SSH…" (⌘⇧O) offers the hosts in `~/.ssh/config` as chips
under a Host field, and a folder field (`~` works). `lathe ssh://host/path` opens one from the
command line. Recent remote projects reopen from the start screen.

## QA, M1b

On the HP under Xvfb, the app opening `/tmp/qa-lathe` over `ssh hp-agent` (the HP to itself).
The shots and recordings are in `~/shots/m1b/` on the HP.

| What | Seen |
| --- | --- |
| Open over ssh | `remote-open.png`: `hp-agent:/tmp/qa-lathe`, main, 1008 files listed in 12 ms |
| Open a file, hover, definition, edit, save | `qa-remote.mp4`, `qa-*.png`: rust-analyzer running on the host; `Instant`'s card; `bind_keys` opens `shell.rs`; "Saved crates/app/src/shell.rs"; `git diff` on the host shows the line |
| The connection drops mid-edit (lathe-remote killed with -9) | `failures.mp4`, `f-1-down.png` (the banner), `f-2-back.png` ("Reconnected", the dirty dot kept), `f-3-saved.png` (the edit saved after), `f-6-server-back.png` (rust-analyzer ready again) |
| A stalled host (SIGSTOP) | `f-4-pending.png` (the pending tab), `f-5-timeout.png` ("hp-agent did not answer in 30 s") |
| The form | `form-*.png`: the config's hosts; an unknown host; `nobody@hp-agent` refused; connecting; open |

## Agent sessions (M2)

The agent comes from `lathe_agents::registry`, so the app names none. Each open project lists the
agent's past sessions (`Backend::sessions`, read off the UI thread); a session open in the window is an
`AgentSession` (`crates/app/src/agent_session.rs`).

- **Where they show.** alex-31's `Sidebar` lists every project with its open and past sessions, each
  with the agent's mark and its status. `AgentPanels` holds a panel per open session, side by side or
  as tabs (⌘\), grouped by project or not (⌘⇧G). The layout, the grouping and the widths are kept in
  the settings file (`panels`) and restored at start.
- **Starting and resuming.** New session: the `+` on a project, ⌘N, or the empty panel's button. A
  past session opens from the sidebar: its history is read first (`Backend::history`), then the agent
  resumes it. A message to a session whose agent stopped (a crash, Stop) resumes it and then goes; a
  message to one that never started (claude missing) tries the start again.
- **Status** (`crates/app/src/status.rs`, pure): Working from a message to the end of its turn,
  NeedsYou while a question waits, Finished (the amber dot) when a turn ends while the reader looks
  at another panel, Idle once they look, Failed with the reason when the agent ends badly, and Idle
  again after Stop.
- **Streaming.** Events land in `EventQueue` on the agent's threads; the queue wakes the session once
  when it goes from empty to not, and the session folds everything waiting and asks for one repaint,
  so a stream costs a repaint a frame. The panel is GPUI's variable-height `list`: only the rows on
  screen and 160 px past them lay out, and after each fold only the rows whose content changed are
  measured again (`list_diff`), so the list keeps its scroll while text streams.
- **The composer.** beui's PromptInput: Enter or ⌘↵ sends, Shift-Enter makes a line, Esc while the
  agent works interrupts its turn, and a press anywhere in the box writes in it. The model picker lists
  the agent's models with their lab's mark (`registry::model_mark`); the mode picker lists its
  permission modes. Both come from `Capabilities`.
- **Approvals.** A question shows `ToolApproval` in its place in the conversation, with the call's
  file and input (long values as code). Allow once, Always allow and Deny answer with the choice of
  that kind the agent offered. **Always allow answers for this session:** Claude Code applies the
  rule it suggested to the running session. A lasting rule would live in the agent's own settings,
  for Claude Code the project's `.claude/settings.local.json`; lathe writes none yet.
- **The header.** The title (a press renames it; the name is kept in the settings file by the
  agent's session id), what the session does, and Stop while its agent runs.
- **Remote projects.** The agent starts through `Project::spawn`, so on an SSH project `claude` runs on
  the host, as a child of `lathe-remote`.
- **Failures.** claude missing on the host: "claude is not installed on this host. Install it there,
  then start a new session." Not logged in: claude's own "Not logged in · Please run /login". A crash
  mid-turn: "Stopped: the agent was stopped by a signal", or, when it left words on stderr, its last
  line, with the tail behind Show details. An interrupt during a question withdraws it; during a tool,
  the tool fails and the turn ends.
- **Not in M2.** The ChangedFiles card at a turn's end needs the turn's line counts, which M3's review
  (`crates/review`'s `TurnTracker`) gives; the PR card needs the forge.

## QA, M2

A real Claude Code session on the HP (claude 2.1.284, logged in), in `/tmp/qa-m2`, and one on
`ssh://hp-agent/tmp/qa-m2-remote`. The shots and recordings are in `~/shots/m2/` on the HP.

| What | Seen |
| --- | --- |
| A first message | `live-1.png`: "hello from lathe" |
| An edit that asks, allowed | `qa-1-ask.png` (ToolApproval for Write, the sidebar says Needs approval), `qa-2-allowed.png` (the file on disk, in the tree, and the reply) |
| Todos and a subagent | `qa-3-work-*.png`: the todo list fills in; a general-purpose subagent runs and ends |
| Interrupt during a question | `qa-4.png`: the question withdrawn, the call failed, Idle |
| Interrupt during a tool | `qa-5.png`: Esc while Bash runs; the call failed, Idle |
| Resume after a restart | `qa-6r.png`: the history is back, and a follow-up is answered from it |
| An SSH project | `qa-7-ssh-ask.png`, `qa-7-ssh-done.png`: `claude` runs as a child of `lathe-remote`; hello.txt lands on the host |
| A crash mid-turn | `qa-8-crash.png`: claude killed with -9; "Stopped: the agent was stopped by a signal" |
| claude missing, not logged in | `qa-9.png` |
| Rename, the mode picker, the single view, restored after a restart | `qa-10.png`, `qa-10-restored-open.png` |
| 2,000 messages | `long-open.png`; the numbers are in docs/performance.md |
| Recordings | `qa-session.mp4`, `qa-work.mp4`, `qa-interrupt.mp4`, `qa-resume.mp4`, `qa-ssh.mp4`, `qa-rename.mp4` |
