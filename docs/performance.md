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

CodeBlock draws its code as one styled text per block, with its gutter as one text and each tinted
line as one band, not one element per line. The story prints its layout node count on the first frame
(`layout nodes first frame N`): 505 for the whole story, with 20 blocks of 25 to 44 lines, the editor and
a 500-row diff. The count before the change is not known exactly, since the story only reported the
highest index of a frame then (3875, with slots that taffy reuses). Frame times on the HP are software
render and say little about the Mac.

The same story on Alex's Mac (Apple M4 Pro, release, commit `b303c2d`), measured by the lead session
with the window in front: the 500-row diff takes 8.33 ms per frame at the median (the 120Hz interval;
p95 9.2 ms), against 12.1 ms before CodeBlock became one text per block. The 5000-row diff takes 9.8 ms
(p95 24 ms), against 12.8 ms before. It still misses 120Hz, and what is left is in the editor and in
FileDiff's rows. Layout nodes: 505, against about 3900. Highlight 0.017 to 0.067 ms per frame, diff
layout 0.53 to 0.74 ms.

A Mac frame run needs the window in front. A covered window is throttled by macOS to 30Hz, and the
frames then take 33 ms whatever the code does. The story counts a frame "over" when it exceeds
`GALLERY_FRAME_MS`, 8.33 ms (120Hz) unless set.

- FileDiff's rows are a virtual list, so its layout no longer grows with the row count. Before, a
  whole frame took 192 ms with 500 rows and 592 ms with 5000.
- The HP renders in software (Mesa's Vulkan under Xvfb), so the whole frame says little about a Mac.

## Themes

Loading every theme at start (`themes::all()`: lathe's two files parsed, eight VS Code files
imported), once, before the first window opens. Load average 0.16:

    cargo test --release -p beui --test theme_bench -- --ignored --nocapture

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| All 10 themes loaded | < 30 ms | 23.26 ms | 23.54 ms | Passes |

A switch reads the loaded theme; it parses nothing.

## Merging

The merge model for one pull request (its blockers, the button and the standing line), which
MergeBox and MergeButton each work out on every render. It reads plain facts the app hands over: no
disk, network or process work. The case is the worst one, every blocker at once. Load average 0.22:

    cargo test --release -p beui --test merge_bench -- --ignored --nocapture

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Merge model, every blocker | < 50 µs | 0.64 µs | 0.68 µs | Passes |
## Agent sessions
What runs when an agent streams. Parsing and mapping run on the session's reader thread, never on the UI
thread. The UI thread only drains a queue once a frame. Machine and method as above; the load average
was 5.7 (the HP is shared). Each number is the median and p95 of 15 runs.

    cargo test --release -p lathe-agents --test perf -- --ignored --nocapture --test-threads=1

The input is the captured runs in `crates/agents/tests/fixtures/claude_code`, repeated to fill 10 MB.

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Parse and map a 10 MB transcript (21,090 lines) | < 250 ms | 40.9 ms (261 MB/s) | 50.2 ms | Passes |
| Parse and map one streamed text delta line | < 10 µs | 0.96 µs | 1.60 µs | Passes |
| Push 100,000 text deltas into `EventQueue` | < 50 ms, 1 event, 1 wake | 3.5 ms (35 ns each), 1 event, 1 wake | 3.8 ms | Passes |

- A model streams about 100 lines a second, so one delta line costs 0.01% of a core.
- `EventQueue` joins the deltas of a block while they wait and wakes the UI once per batch. A stream costs
  one wake and one repaint a frame, however many tokens land in it.
- A tool result over 64 KiB is cut to its head, so one huge result cannot reach the UI whole. The
  mapper's memory per session is the todo list, the open blocks and the running calls.
- A live session, on the HP: the reader and writer threads sleep on the pipes. A `send` only queues a
  line. It never waits for the agent.
## The forge
What runs when lathe reads GitHub. Every call runs off the UI thread. The cases below run on answers held
in memory, so they measure lathe's own work (the request, the JSON, the mapping) and not the network.
Machine and method as above; load average 1.8, 20 runs.

    cargo test -p lathe-forge --release --test perf -- --ignored --nocapture --test-threads=1

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Lookup of 50 `#N`: build the one request, read and map the answer | < 1 ms | 0.17 ms | 0.22 ms | Passes |
| Read a 300-file pull request with 500 threads and 5,000 comments (12.7 MB of JSON, 16 pages) | < 100 ms | 37.4 ms | 40.6 ms | Passes |
| File 500 pull requests into Courts and map them to rows | < 5 ms | 0.82 ms | 0.90 ms | Passes |

The 5,000 comments are the recorded 1.2 KB comment, so the payload is larger than most real ones. Before
the parser stopped copying each page's subtree, the same case took 84 ms at a load average of 16.

Live, against GitHub through `gh` on the HP (`cargo test -p lathe-forge --test live -- --ignored
--nocapture`, one run each, so not medians):

| Case | Time |
| --- | --- |
| 50 pull numbers of `oven-sh/bun`, one request | 749 ms |
| The reader's working set: ten searches at once, 54 pull requests | 5.2 s |

The working set is the slow one, and it is network and GitHub's own time: each search reads the checks of
every row. It never runs on the UI thread, and the UI can show the remembered list first. Fewer fields per
row would cut it; that is open.


## The app

The `lathe` binary on the HP under Xvfb (Mesa's software Vulkan), release. The load average came
from alex-31's builds running at the same time.

| Case | Target | Median | Worst | Runs | Load | Result |
| --- | --- | --- | --- | --- | --- | --- |
| First frame, from process start, a project open | < 300 ms | 220.4 ms | 373.6 ms | 11 | 14.2 | Passes |
| Listing 10,000 files (`Project::list`, a .gitignore, 2,000 ignored) | < 200 ms | 9.79 ms | 10.43 ms (p95) | 20 | 3.7 | Passes |
| The app's tree of 10,000 files: listing and building the rows model | < 200 ms | 11 ms | 13 ms | 7 | 14 | Passes |

    cargo test --release -p lathe-project --test list_bench -- --ignored --nocapture
    LATHE_TIMINGS=1 target/release/lathe <folder>     # prints the first frame's time
    target/release/lathe /tmp/qa-10k                    # the status line says "listed in N ms"

- The listing and the tree build run on a background thread. The tree draws as a virtual list, so
  only the rows on screen lay out.
- The first frame waits for no disk work: the settings file is read before the event loop, and the
  project lists after the window shows.

## Review
What runs when a turn ends and when the reader works on it. All of it runs off the UI thread. Machine and
method as above; load average 4.4, 15 runs (7 for the disk case).

    cargo test -p lathe-review --release --test perf -- --ignored --nocapture --test-threads=1

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| A turn of 200 files, one of 20,000 lines: all hunks, from texts in memory | < 100 ms | 20.2 ms | 23.4 ms | Passes |
| The same, through `TurnTracker::finish` on real files with git (status, hashes, reads, hunks) | < 100 ms | 55.5 ms | max 57.2 ms | Passes |
| One edit in a 20,000-line file, diffed | < 5 ms | 1.7 ms | 2.0 ms | Passes |
| The agent edits again with the review open: `rebased_on` | < 5 ms | 2.5 ms | 2.7 ms | Passes |
| One decision on the merged text of that file | (none) | 0.66 ms | 0.84 ms | |
| One keystroke moving the hunks (`track_edit`) | (none) | 0.70 ms | 0.80 ms | |
| A 20,000-line file with 500 hunks | < 50 ms | 26.3 ms | 28.2 ms | Passes |

The first run of the disk case took 250 ms: it read the last commit's text of each file git found with one
`git show` a file. One `git cat-file --batch` for all of them brought it to 55 ms.

## Sidebar and panels
The gallery's "Sidebar" and "Panels" stories under a measuring run (`GALLERY_SCROLL=1`), release, on the
HP. The HP draws in software, so a whole frame there is 48 ms whatever the code does; the numbers to read
are our own layout, prepaint and paint, and the layout node count of the first frame. A whole-frame run on
a Mac needs the window in front (see above).

    GALLERY_STORY=Sidebar GALLERY_SCROLL=1 SIDEBAR_PROJECTS=50 SIDEBAR_SESSIONS=40 target/release/beui-gallery
    GALLERY_STORY=Sidebar GALLERY_SCROLL=1 SIDEBAR_PROJECTS=50 SIDEBAR_SESSIONS=40 SIDEBAR_OPEN=1 ...
    GALLERY_STORY=Panels GALLERY_SCROLL=1 PANELS=12 target/release/beui-gallery
    GALLERY_STORY=Panels GALLERY_SCROLL=1 PANELS=12 GALLERY_SWITCH=1 ...

| Case (300 frames, load average 9) | Target | Layout and prepaint, median | p95 | Paint, median | Layout nodes, first frame |
| --- | --- | --- | --- | --- | --- |
| Sidebar, 50 projects and 2,000 sessions, folded | frame under 8.3 ms | 1.10 ms | 2.11 ms | 0.36 ms | 105 |
| The same with every fold open (every session a row) | frame under 8.3 ms | 1.11 ms | 1.87 ms | 0.31 ms | 105 |
| Panels, 12 side by side, scrolled a frame at a time | frame under 8.3 ms | 0.66 ms | 1.52 ms | 0.64 ms | 208 |
| The same, switching the layout every 30th frame | one frame | 0.56 ms | 0.88 ms | 0.47 ms | 208 |

The node count does not grow with the data: the list builds the rows in view, and the strip builds the
columns in view and a margin. The whole-frame numbers on the Mac are for the lead to measure. The
switching run counts the frames with a switch apart ("frame with a switch"); on the HP they took the same
48 ms as the rest.

## Remote projects

`RemoteProject` over a real `ssh` from the HP to itself (`hp-agent`, loopback through sshd), the
release lathe-remote, `/tmp/qa-lathe` (1,009 files). Load average 9 to 11. Three runs of 20
samples each; the LAN number waits on a run from the Mac to the HP.

    LATHE_REMOTE_DIR=… LATHE_TEST_SSH_HOST=hp-agent LATHE_TEST_SSH_ROOT=/tmp/qa-lathe \
        cargo test --release -p lathe-remote --test over_ssh -- --ignored --nocapture remote_costs

| Case | Target | Result | Passes |
| --- | --- | --- | --- |
| Connect, the copy already there (probe, check, dial, hello) | < 3 s | 420 to 476 ms | Yes |
| Connect, uploading a new copy first | < 5 s | 966 ms | Yes |
| File open: reading a 10,000-line file (212 KB) | < 150 ms on a LAN | 1.17 to 2.04 ms median, 1.39 to 3.12 ms p95 (loopback) | Yes, on loopback |
| Listing 1,009 files | < 500 ms | 3.89 to 4.82 ms median, 4.86 to 6.36 ms p95 | Yes |
