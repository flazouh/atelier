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
