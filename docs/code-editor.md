# The code editor

The surface where the user reads the agent's edits, accepts or rejects them, and writes code. Three stages,
each one shippable on its own.

Written at commit `5bd2b0e`. All three stages are built. Values are exact; take them from here, not
from memory.

## What already exists, and what we add

gpui-base and gpui-component ship the hard parts, so lathe reskins them rather than rewriting:

| Need | Where it lives | What lathe does |
|---|---|---|
| Editing, selection, undo, IME | `gpui_base::input` (`EditorState`, 9.4k lines) | Reuse. |
| Syntax highlighting | `gpui-component/tree-sitter-*` features | Turn on the languages, restyle the token colors. |
| Line numbers, folding, soft wrap | `EditorState::line_number`, fold candidates | Reuse, restyle the gutter. |
| Diagnostics model and hover | `gpui_base::input::{Diagnostic, DiagnosticSet, DiagnosticSeverity}`, `DiagnosticPopover` | Feed it from LSP, restyle. |
| LSP client | nothing | Stage C writes one. `lsp-types` 0.97 is already in the lock. |
| Agent hunks with accept and reject | nothing | Stage A writes it. |

## What the live server taught us

`cargo run -p lathe-lsp --example probe -- <root> <file>` prints a server's traffic. Two facts came out
of it, and both are now handled:

- rust-analyzer publishes twice for one document version: an empty set while it indexes, then the real
  one. An empty set is also how a server says a file is clean, so nothing in the protocol tells them
  apart. A client that takes the first set reports "nothing is wrong" about text the server has not read
  yet. `wait_for_diagnostics_at` waits for the first matching set, then keeps taking later ones until
  600ms passes with none, and returns the last.
- A `didChange` that carries the same text makes the server publish nothing. Bumping the version there
  leaves the caller waiting for an answer that never comes, so only changed text is sent.

## Stage A: the hunk review

The thing the user sees while the agent edits a file. One `HunkReview` per file.

- **Rows.** Mono at `text-xs`, line height 20px, a 36px gutter of old and new numbers, then a 12px sign
  column, then the code. Added rows wash with `theme.diff_line(true)`, removed rows with
  `diff_line(false)`, both 7% as `FileDiff` already does. Context rows have no wash.
- **Per hunk bar.** It sits at the hunk's top right, on the row band, and holds Accept and Reject as
  `ButtonSize::Chip`, plus `Kbd` hints `⌘↵` and `⌘⌫`. It fades in on hover over the hunk or when the hunk
  is the current one, over `duration::MORPH` (180ms).
- **Header.** The path in mono, `+n −m` in the muted diff colors, the count of hunks left, then Accept all
  and Reject all.
- **Resolve animation.** This is the heart of it. Accepting a hunk collapses its removed rows to zero
  height while their wash fades, and the added rows drop their wash and settle into plain code. Rejecting
  does the mirror. Both run over `duration::RESOLVE` (260ms) on `ease::MORPH`, and the rows below slide up
  as the height closes. Under Reduce Motion the rows vanish at once with no slide.
- **Arrival.** A hunk the agent has just written enters with `Entrance`, staggered 35ms, as the chat items
  do.
- **State.** `HunkReview` takes `Vec<Hunk>`; each `Hunk` has an id, its rows, and a `HunkState` of
  `Pending`, `Accepted` or `Rejected`. The component animates and reports; the caller owns the file.

Pure functions to test first: `hunk_stats` (added and removed per hunk and per file), `resolve_rows` (which
rows survive an accept or a reject), and `pending_count`.

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

- One `LspClient` per server, talking JSON-RPC over stdio to `rust-analyzer` (and later `tsgo` or
  `typescript-language-server`). `lsp-types` 0.97 supplies the wire types.
- It handles `initialize`, `initialized`, `textDocument/didOpen`, `didChange`, `didSave`, and
  `shutdown`; it reads `textDocument/publishDiagnostics` and answers `textDocument/definition` and
  `textDocument/hover`.
- The editor pushes every edit as a `didChange` after a 150ms idle, and turns each published diagnostic
  into a `gpui_base::input::Diagnostic`.
- Go to definition: `⌘` and a click, or `F12`, sends `definition` and opens the answer. When the server has
  no answer, the status row says so; it never guesses a location.
- The crate states plainly what it cannot do: no completion, no rename, no code actions yet.

## Checks

- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo build -p beui-gallery`.
- Gallery stories "Hunks" and "Editor", captured on `hp-agent` in both themes.
- Stage A resolve: capture at 0, 130 and 260ms and confirm the rows close rather than jump.
- Stage C: `LATHE_REQUIRE_LSP=1 cargo test -p lathe-lsp --test rust_analyzer` on a box with the server,
  so a missing server fails rather than skipping. Passed on `hp-agent` against rust-analyzer 1.98.1.
- The Editor story, driven on `hp-agent`: Check reported "2 problems, first: mismatched types, expected
  `u32`, found `&str`" and the editor underlined the range; Go to definition on `width()` answered
  "defined on line 5". Both were captured as screenshots.
- Motion under software Vulkan redraws about 19 times a second, so a 260ms resolve gets 5 frames and
  reads as a jump in a recording. Use `tools/gallery-record-linux.sh` and count changed frames rather
  than trusting a tile of screenshots.
