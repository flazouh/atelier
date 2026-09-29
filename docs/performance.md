# Performance

Every path lathe adds gets a number here: the median and p95 of 20 runs, release, on the HP, against a
target. Nothing runs disk, network, process or parse work on the UI thread.

## Syntax colours and the editor

Syntax colours come from gpui-component's tree-sitter `SyntaxHighlighter` in two places:

- The editor drives its own highlighter (gpui-component's `input_adapter.rs`). Its UI thread never
  parses: a keystroke only applies the edit, which moves the old colours with the text, and every
  parse runs on a background thread (`vendor/gpui-component/PATCHES.md`, patch 2).
- Diffs, code blocks and the pull request view go through `crates/beui/src/syntax.rs`. Every text parses
  on a background thread too. Each background thread keeps one highlighter per language, because
  building one compiles its queries: about 50 ms for Rust on the HP, against 0.14 ms to parse a
  25-line block with a warm one. While a text parses, its slot keeps the colours of its last text on
  each unchanged row, so a streamed block does not flash plain. A slot's new text drops the parse of
  its old one.

The editor's two patches to gpui-component:

1. After an edit, the injection layers (Rust's macro bodies, Markdown's fences) are moved and
   re-queried only where the tree changed, instead of rebuilt over the whole file.
2. The parse moved off the UI thread for every text size, one at a time per editor: the keystrokes
   during a parse coalesce into the next one, and a parse for an older text is dropped. Until one
   lands, each old colour stands where its text went. The background parse updates the injection
   layers in place too, over one span that all the coalesced edits wrote.

Machine: `hp-agent`, Intel i5-10500T (6 cores, 12 threads, 2.3 GHz), release build, at commit
`e0e79a4`. The HP is shared with CI runners; this run started at a load average of 5.6 and ended at
10.3. Each number is the median and p95 of 20 runs.

    cargo test --release -p beui --test highlight_bench -- --ignored --nocapture --test-threads=1

The editor now, at commit `11e1eea` and a load average of 2.1 to 3.9:

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Keystroke on the UI thread, flat 10k-line Rust (about 900 top-level functions) | < 1 ms | 0.27 ms | 0.31 ms | Passes |
| The same, the functions in modules of 50 | < 1 ms | 0.29 ms | 0.33 ms | Passes |
| The same, a 300-line file | < 1 ms | 0.24 ms | 0.28 ms | Passes |
| Colour latency, flat 10k-line Rust | none yet | 4.40 ms | 6.23 ms | |
| The same, the functions in modules of 50 | none yet | 0.88 ms | 0.97 ms | |
| The same, a 300-line file | none yet | 0.86 ms | 1.77 ms | |
| First parse of the 10k-line file and visible styles | < 50 ms, off the UI thread | 132.2 ms | 134.6 ms | Off the UI thread; over the time |
| Visible styles for a scroll step | < 0.5 ms | 0.40 ms | 0.45 ms | Passes |

- "Keystroke on the UI thread" is what the UI thread does now: `edit_tree`, then the visible rows'
  styles from the moved tree.
- "Colour latency" is the edit, the background parse with its injection layers, taking it, and the
  visible styles, run in a row. The editor adds a hop to a background thread and back, and the next
  frame. Before the background parse updated the layers in place it rebuilt them all: 60.4 ms for the
  flat file, 51.1 ms in modules, 3.0 ms for 300 lines, at a load average of 5.6 to 10.
- For the flat file, what is left is tree-sitter's incremental parse and `changed_ranges`, which walk
  the root's about 900 children; in modules the same text takes 0.88 ms.
- The first parse is a full pass, so neither patch changes it: about 50 ms of it compiles the highlight
  and injection queries, the rest parses the file and its first 512 macro layers.

The same keystroke parsed on the spot, as the editor did before patch 2 (after patch 1):

| Case | Before patch 1 | Median | p95 |
| --- | --- | --- | --- |
| Flat 10k-line Rust | 49.4 ms | 7.43 ms | 9.15 ms |
| Typed inside a macro that has a layer | | 6.13 ms | 8.02 ms |
| The same file with no macros | 21.0 ms | 5.61 ms | 7.57 ms |
| The functions in modules of 50 | | 0.85 ms | 0.99 ms |
| 10k-line Go, which has no injections | 1.06 ms | 1.15 ms | 1.59 ms |

A parse on the spot costs what tree-sitter's incremental parse and `changed_ranges` cost, and both
walk the root's children: 1.8 to 3.2 ms and 1.1 to 1.6 ms for the flat file, at a load near 4. That
is why the editor no longer parses on the UI thread.

Diffs and code blocks:

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Diff: 5k rows, both sides parsed and styled | < 50 ms, off the UI thread | 20.8 ms | 27.5 ms | Passes |
| Diff: 5k rows, a frame on the UI thread | < 0.5 ms | 0.16 ms | 0.18 ms | Passes |
| 20 code blocks of 25 lines, all parsed | off the UI thread | 2.93 ms | 4.38 ms | Passes |
| One 25-line block, warm highlighter | | 0.14 ms | 0.17 ms | |
| One 25-line block, new highlighter | | 51.5 ms | 59.8 ms | |
| 3k-line TypeScript, parsed and styled | | 15.9 ms | 18.5 ms | |
| 2k-line Zig, parsed and styled | | 9.8 ms | 15.6 ms | |

Frames, from the gallery's "Highlight load" story: a 10k-line Rust file in a 480 px editor and a diff
of 500 or 5000 rows in a 560 px view on the left, 20 code blocks on the right, all scrolled 48 px a
frame for 300 frames under Xvfb, at a load average of 12 to 13. "Diff layout" is FileDiff's
request_layout and prepaint; "diff paint" is building its scene on the CPU.

    DISPLAY=:97 GALLERY_STORY="Highlight load" GALLERY_SCROLL=1 LOAD_DIFF_ROWS=5000 target/release/beui-gallery

| Measure | 500 rows, median | 5000 rows, median | 5000 rows, p95 | 5000 rows, max | Over 8 ms, 5000 rows |
| --- | --- | --- | --- | --- | --- |
| Diff layout | 1.06 ms | 1.30 ms | 1.91 ms | 2.56 ms | 0 of 300 |
| Diff paint | 0.22 ms | 0.22 ms | 0.32 ms | 6.31 ms | 0 of 300 |
| Time in `syntax::highlight` per frame | 0.031 ms | 0.093 ms | 0.262 ms | 0.319 ms | 0 of 300 |
| Whole frame | 64.0 ms | 64.0 ms | 80.1 ms | 209 ms | 300 of 300 |

- FileDiff's rows are a virtual list, so its layout no longer grows with the row count. Before, a
  whole frame took 192 ms with 500 rows and 592 ms with 5000.
- The HP renders in software (Mesa's Vulkan under Xvfb), so the whole frame says little about a Mac.
