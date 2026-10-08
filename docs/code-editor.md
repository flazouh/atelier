# The code editor

The surface where the user reads the agent's edits, accepts or rejects them, and writes code. Three stages,
each one shippable on its own.

Written at commit `5bd2b0e`. All three stages are built. Values are exact; take them from here, not
from memory.

## What already exists, and what we add

gpui-base and gpui-component ship the hard parts, so atelier reskins them rather than rewriting:

| Need | Where it lives | What atelier does |
|---|---|---|
| Editing, selection, undo, IME | `gpui_base::input` (`EditorState`, 9.4k lines) | Reuse. |
| Syntax highlighting | `gpui-component/tree-sitter-*` features | Turn on the languages, restyle the token colors. |
| Line numbers, folding, soft wrap | `EditorState::line_number`, fold candidates | Reuse, restyle the gutter. |
| Diagnostics model and hover | `gpui_base::input::{Diagnostic, DiagnosticSet, DiagnosticSeverity}`, `DiagnosticPopover` | Feed it from LSP, restyle. |
| LSP client | nothing | Stage C writes one. `lsp-types` 0.97 is already in the lock. |
| Agent hunks with accept and reject | nothing | The inline review writes it (`docs/inline-review.md`). |

## What the live server taught us

`cargo run -p atelier-lsp --example probe -- <root> <file>` prints a server's traffic. Two facts came out
of it, and both are now handled:

- rust-analyzer publishes twice for one document version: an empty set while it indexes, then the real
  one. An empty set is also how a server says a file is clean, so nothing in the protocol tells them
  apart. A client that takes the first set reports "nothing is wrong" about text the server has not read
  yet. `wait_for_diagnostics_at` waits for the first matching set, then keeps taking later ones until
  600ms passes with none, and returns the last.
- A `didChange` that carries the same text makes the server publish nothing. Bumping the version there
  leaves the caller waiting for an answer that never comes, so only changed text is sent.
- A request that lands just after a `didChange` can get `ContentModified` (-32801), and a busy server
  answers `ServerCancelled` (-32802). `until_settled` asks again with a short backoff.
- Under load, rust-analyzer republished an old result, and `cargo check` results for the file on disk,
  tagged with the newest version, then sent nothing more. So the editor pulls diagnostics
  (`textDocument/diagnostic`), which the server works out on the text it has. It answers a pull fully
  only when the client says it pulls, and while it loads the workspace it answers empty or `null`; the
  worker waits for `experimental/serverStatus` to say `quiescent` before it pulls.
- Positions are negotiated as UTF-32, so they count characters as gpui-base does.

The editor story wires it the way Zed does: Cmd-hover underlines, Cmd-click and F12 jump, a hover card
rests on a symbol, and problems are pulled on open and 150ms after typing pauses. `LspWorker` owns the
server on its own thread, so no request blocks the window, and a newer check replaces a queued one.

## Stage A: the hunk review (removed)

Stage A was a separate diff pane, `HunkReview`, with Accept and Reject on each hunk, for while the
file's editor was locked read-only. The inline review (`docs/inline-review.md`) replaced that lock, so
the pane had no use left and was deleted on 2026-09-28. Accepting and rejecting now happen in one place,
the editor, for an open file and for a multi-file review alike. `FileDiff` stays, for a read-only diff
in a chat message.

## Stage B: the editor

`CodeEditor` wraps gpui-component's `Editor` and `EditorState`.

- Geist Mono at `text-xs`, line height 20px, the gutter in `muted_foreground` at 40%, the current line
  washed with `card`.
- Languages on at first: Rust, TypeScript, JavaScript, JSON, Markdown, Go, Python, TOML. Each is a
  `gpui-kit` feature, so the build stays honest about what it can highlight.
- Token colors come from the muted palette, not Tailwind: keywords take `theme.info`, strings
  `theme.success`, numbers `theme.warning`, comments `muted_foreground` at 60%, types the foreground.
- Diagnostics underline their range in `theme.danger` for an error and `theme.warning` for a warning, and
  the gutter shows a filled `StatusMark` per line, from the same ramp the tool rows use.
- Read only while a hunk review is open on the same file, so the two never fight over the buffer.

## Stage C: LSP

A new crate, `crates/lsp`, with no UI.

- One `LspClient` per server, talking JSON-RPC over stdio to `rust-analyzer`, `tsgo`, `ty` and the
  others in the table below. `lsp-types` 0.97 supplies the wire types.
- It handles `initialize`, `initialized`, `textDocument/didOpen`, `didChange`, `didSave`, and
  `shutdown`; it reads `textDocument/publishDiagnostics` and answers `textDocument/definition` and
  `textDocument/hover`.
- The editor pushes every edit as a `didChange` after a 150ms idle, and turns each published diagnostic
  into a `gpui_base::input::Diagnostic`.
- Go to definition: `⌘` and a click, or `F12`, sends `definition` and opens the answer. When the server has
  no answer, the status row says so; it never guesses a location.
- The crate states plainly what it cannot do: no completion, no rename, no code actions yet.

## Stage D: any language, and references

The editor must work the same way for Rust, TypeScript, JavaScript, Python, Go and Java, and for a
new language by adding data, not code. Nothing below the registry may name a language or a server.

**Registry (`crates/lsp/src/servers.rs`, data only).** One `ServerSpec` per server: a name, the LSP
language ids it serves, the program and its arguments, the files that mark a project root, and the
install hint the status line shows when the program is missing. Built in:

| Language ids | Server | Root markers |
|---|---|---|
| `rust` | `rust-analyzer` | `Cargo.toml` |
| `typescript`, `typescriptreact`, `javascript`, `javascriptreact` | `tsgo --lsp --stdio` (TypeScript 7, native) | `tsconfig.json`, `jsconfig.json`, `package.json` |
| `python` | `ty server` (Astral, Rust) | `pyproject.toml`, `ty.toml`, `setup.py`, `requirements.txt` |
| `go` | `gopls` | `go.mod` |
| `java` | `jdtls` | `pom.xml`, `build.gradle`, `settings.gradle` |

- `language_id(path)` maps a file extension to its standard LSP language id.
- `find_program` looks in `PATH`, then in `~/.cargo/bin`, `~/.local/bin`, `~/go/bin`,
  `/opt/homebrew/bin` and `/usr/local/bin`, since an app opened from the Finder has almost no `PATH`.
- `find_root` walks up from the file to the nearest root marker; with none, the file's directory.

**Worker, one per (server, root).** It keeps many documents open: a request carries its path and text,
and a path the server has not seen is opened with `didOpen` and its language id. It reads the
server's capabilities once and adapts:

- Diagnostics are pulled when the server offers `diagnosticProvider`, and otherwise taken from the
  newest published set for the document's current version, waited for with the settle rule above.
- Positions: the client offers UTF-32 and UTF-16. The worker converts at its boundary, so every
  position it takes or returns counts characters, which is what gpui-base uses. A caller never sees
  the encoding.
- rust-analyzer's `serverStatus` is read when sent and ignored otherwise.

**Go to definition, then references, as Zed does.** One worker question, `navigate(path, text,
position)`, asks `textDocument/definition`. If the answer is empty or only the symbol under the caret,
it asks `textDocument/references` without the declaration. The answer says which kind it is and lists
locations, each with its line's text for display.

- Cmd-hover underlines the symbol whenever the answer is not empty, so a definition is clickable too.
- One location: Cmd-click and F12 move the caret there.
- Several: a references list under the editor, one row per location (file, line, the line's text);
  a click on a row in this file moves the caret, in another file names it in the status line.
- Shift-F12 asks for references directly.

**Built as specified, plus what the review asked for.**

- `Workers` (`crates/lsp/src/pool.rs`) keeps one worker per (server, project root), shared by every
  file of that project, and starts a new one when a server has stopped. The live tests prove two files
  of one crate share a server and another crate gets its own.
- Files are compared by canonical path, never by URI string: rust-analyzer leaves `@` raw and
  vscode-uri servers write `%40`.
- `Published` (`crates/lsp/src/published.rs`) is the one rule for a pushed set: a versioned set is
  current from its version on, and an unversioned one only until the next change, since every sync
  forgets the document's set.
- Requests a server sends are answered: no settings, yes to registrations, the root for workspace
  folders, and method-not-found for the rest, so no server waits on an answer that never comes.
- Only the newest navigate, hover and diagnostics question per file runs; older ones are superseded.
- Java has a registry row and a live test that runs when `jdtls` is installed; it is unproven until then.
- Not handled yet: Windows paths, and servers whose arguments depend on the root.

**What the servers taught us.**

- The fastest server per language, chosen on 2026-09-28. TypeScript 7 (`typescript@7.0.2`) is the
  native Go compiler, and it serves LSP itself, so there is no Node.js and no tsserver. For Python,
  ty 0.0.84 and pyrefly 1.3.1 were timed on `dev-host` with a full `check`: ty took 0.17s on `rich`
  against 0.28s, and 1.58s on `django` against pyrefly's 1.72s at its `default` preset. ty is still
  0.0.x, so it can change under us.
- tsgo sends its own requests with string ids (`"ts1"`) and answers nothing until it hears back. The
  read loop answers every server request itself, because the worker may be blocked on tsgo's answer.
  When the worker answered, the first question after startup waited out the 60s timeout.
- tsgo refuses a diagnostic pull that carries `"identifier": null`, which `lsp-types` 0.97 writes for
  an unset field, so atelier sends its own params with only the document.
- gopls and tsgo count columns in UTF-16; rust-analyzer takes UTF-32. The worker covers both, and the live tests
  put an emoji before the call to prove the conversion.

**Proof.** A live test per server, skipped when its program is missing unless `ATELIER_REQUIRE_LSP` names
it (`ATELIER_REQUIRE_LSP=rust,typescript,python,go`): from a call, definition lands on the declaration;
on the declaration, the fallback lands on the call; a type error is reported; hover names the symbol.
The gallery's Editor story has a tab per language with a fixture project on disk.

## Stage E: servers come with the app

A user installs atelier and nothing else. When a file's server is missing, atelier downloads it once,
as Zed does, and every later start uses that copy.

**Order.** A server the user installed wins, so their version and their settings apply. Then the
copy atelier already downloaded. Then a download. `ATELIER_OFFLINE=1` stops at the second step, and the
status line shows the install hint as before.

**Where.** `~/Library/Application Support/atelier/servers` on macOS, `$XDG_DATA_HOME/atelier/servers`
(else `~/.local/share/atelier/servers`) on Linux, or `ATELIER_SERVERS_DIR`. Each download lives in
`<name>/<version>/`. It is unpacked in a staging folder beside it and renamed into place only after
its checksum matched, so a folder that exists is complete, and two ateliers racing both end up using it.

**What, pinned in the registry.** Every file has a fixed version and a SHA-256 taken when it was
pinned; a file that does not match is deleted and the server reports it as a failed download.

| Server | Download | Runs as |
|---|---|---|
| `rust-analyzer` | the `.gz` binary for the platform from its GitHub release | the binary |
| `tsgo` | the platform's `@typescript/typescript-<platform>` npm tarball | `lib/tsc --lsp --stdio` |
| `ty` | the platform's tarball from its GitHub release | `ty server` |
| `gopls` | `go install` with `GOBIN` in atelier's folder | the binary; needs Go, which a Go project has |
| `jdtls` | not downloaded yet: it needs a Java runtime too | the install hint |

- A user's `tsc` on the PATH is most often TypeScript 5, which has no `--lsp`, so atelier looks for
  `tsgo`, the name the native preview installed it under, and otherwise downloads TypeScript 7.
- No server needs Node.js now. The Node.js pin and the npm install path stay for a future npm server.
- Downloads use the system's `curl`, `tar` and `gzip`, which macOS and Linux both ship. The checksum is
  computed in Rust.
- Platforms: `aarch64` and `x86_64` on macOS and Linux. Anywhere else the install hint shows.
- While it downloads, the status line says so, with the server and its version.
- One download runs at a time, and never while holding the lock that starts servers, so a download
  never stops a server that is already installed from starting.

**Proof.** Unit tests serve fixtures from `file://` URLs: a binary and a Node server install and run,
a wrong checksum leaves nothing behind, a finished copy is used without any download, offline stops
before the network, and a server on the search path wins. A live test, run when
`ATELIER_TEST_DOWNLOADS=1`, downloads the real rust-analyzer, tsgo, ty and gopls into an empty folder
and runs every live check on each. It searches only Go's own folder for `go`, never a shared one such
as `~/.local/bin`, which would hold installed servers.

## Performance

The numbers and targets are in [performance.md](performance.md).

## Checks

- `tools/check.sh` on `dev-host`: the workspace tests with live rust-analyzer, tsgo, ty and gopls,
  clippy, the gallery
  build, and the tests of the patched `vendor/gpui-base`, which the workspace excludes.
- Gallery stories "Hunks" and "Editor", captured on `dev-host` in both themes.
- Stage C: `ATELIER_REQUIRE_LSP=1 cargo test -p atelier-lsp --test rust_analyzer` on a box with the server,
  so a missing server fails rather than skipping. Passed on `dev-host` against rust-analyzer 1.98.1.
- The Editor story, driven on `dev-host`: Check reported "2 problems, first: mismatched types, expected
  `u32`, found `&str`" and the editor underlined the range; Go to definition on `width()` answered
  "defined on line 5". Both were captured as screenshots.
- Motion under software Vulkan redraws about 19 times a second, so a 260ms resolve gets 5 frames and
  reads as a jump in a recording. Use `tools/gallery-record-linux.sh` and count changed frames rather
  than trusting a tile of screenshots.
