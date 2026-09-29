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

## Tasks
The gallery's "Tasks" story under a measuring run, release, on the HP: a list of 5,000 tasks scrolled a
frame at a time, with a filter (Priority: Urgent) applied at frame 75 and taken off at frame 225. The
frames with a filter are counted apart. The HP draws in software (48 ms a frame whatever the code does), so
the numbers to read are our own layout, prepaint and paint. Load average 14 to 18 during these runs, so the
worst frames vary from run to run.

    GALLERY_STORY=Tasks GALLERY_SCROLL=1 TASK_COUNT=5000 target/release/beui-gallery

| Case (300 frames) | Target | Layout and prepaint, median | p95 | Paint, median | Layout nodes, most |
| --- | --- | --- | --- | --- | --- |
| List, 5,000 tasks, scrolled | layout under 8 ms | 2.4 ms | 4.0 ms | 1.7 ms | 493 |
| The frame that applies a filter | one frame | 2.0 ms | 5.0 ms | | |

Filtering and grouping 5,000 tasks in `set_filters` took 0.3 to 0.9 ms. The node count does not grow with
the data. A few frames in a run pass 8 ms in layout (4 to 21 in three runs; the index differs each time),
which the run shows only under load. The Mac row is alex-9c's run (M4 Pro, release, window in front):

| Case (5,000 tasks) | Frame, median | p95 | Layout and prepaint, median | max | Paint, median | Layout nodes |
| --- | --- | --- | --- | --- | --- | --- |
| List, Mac | 8.33 ms (120 Hz) | 9.0 ms | 1.0 ms | 2.7 ms, 0 frames over budget | 0.8 ms | 126 to 493 |

The board, on the HP (`TASKS_VIEW=board`, scrolled up and down and sideways every frame, a filter at frames
75 and 225): layout and prepaint median 3.2 ms, p95 11.5 ms, 62 of 298 frames over 8.33 ms under a load
average of 8; the frames with a filter took 2.7 ms in layout; paint median 1.1 ms; at most 827 layout
nodes. A column's card list reads shared data (`Rc`), so a frame clones a pointer and the cards in view,
not the column; before that change the median was 4.0 ms. The Mac row is alex-9c's run (M4 Pro, release, window in front, 5,000 tasks, scrolling every way):

| Case (5,000 tasks) | Frame, median | p95 | Layout and prepaint, median | p95 | max | Paint, median | Layout nodes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Board, Mac | 8.33 ms (120 Hz) | 8.85 ms | 1.09 ms | 1.49 ms | 1.88 ms, 0 frames over budget | 0.42 ms | 119 to 827 |

So the HP's spikes were load.

## Our own agent
`crates/agents/tests/own_perf.rs`, release, on the HP (shared; the load average was 24). Each number is the
median and p95 of 15 runs. No network: the model is a scripted stand-in, so the numbers are lathe's own work.

    cargo test -p lathe-agents --release --test own_perf -- --ignored --nocapture --test-threads=1

| Case | Target | Median | p95 | Result |
| --- | --- | --- | --- | --- |
| Parse a 10 MB Messages API stream (83,890 events): SSE, JSON, the reply | < 500 ms, < 20 µs an event | 133 ms (75 MB/s, 1.6 µs an event) | 241 ms | Passes |
| 100 tool calls (`read` of a small file), instant model, the record not written | < 1 ms a call | 4.5 ms in all, 45 µs a call | 6.5 ms | Passes |
| The same with the record written after every message (a whole-file write) | | 10.6 ms in all, 106 µs a call | 14.3 ms | |

- The loop's own cost per tool call is 45 µs: the request, the events, the permission check, the tool and the
  results. The record adds about 60 µs a call here, and grows with the conversation because a write is the
  whole file.
- A model streams about 100 events a second, so a stream costs 0.016% of a core.
- An earlier build made the record's folder (a process) after every message and took 3 ms a call. The folder is
  made once a session now.
- The real API is not in these numbers: a call takes seconds, and the stall is the model's.

## Remote projects

`RemoteProject` over a real `ssh` from the HP to itself (`hp-agent`, loopback through sshd), the
release lathe-remote, `/tmp/qa-lathe` (1,009 files). Load average 9 to 11. Three runs of 20
samples each. The LAN row is alex-9c's run from Alex's Mac to the HP over Wi-Fi, a clone of
1,150 files, the same bench.

    LATHE_REMOTE_DIR=… LATHE_TEST_SSH_HOST=hp-agent LATHE_TEST_SSH_ROOT=/tmp/qa-lathe \
        cargo test --release -p lathe-remote --test over_ssh -- --ignored --nocapture remote_costs

| Case | Target | Result | Passes |
| --- | --- | --- | --- |
| Connect, the copy already there (probe, check, dial, hello) | < 3 s | 420 to 476 ms | Yes |
| Connect, uploading a new copy first | < 5 s | 966 ms | Yes |
| File open: reading a 10,000-line file (212 KB) | < 150 ms on a LAN | 1.17 to 2.04 ms median, 1.39 to 3.12 ms p95 (loopback) | Yes, on loopback |
| Listing 1,009 files | < 500 ms | 3.89 to 4.82 ms median, 4.86 to 6.36 ms p95 | Yes |
| LAN, Mac to HP over Wi-Fi: connect, the copy there | < 3 s | 0.68 to 0.77 s | Yes |
| LAN: connect, uploading a new copy first | < 5 s | 1.4 s | Yes |
| LAN: file open, 212 KB | < 150 ms | 19 to 49 ms median | Yes |
| LAN: listing 1,150 files | < 500 ms | about 20 ms | Yes |

## Agent sessions in the app

A 2,000-message session (a synthetic transcript in claude's own format, 1,000 questions and 1,000
answers of about 380 characters) opened from the sidebar, then scrolled with the wheel for 700 frames,
in the release app under Xvfb at 1400×860, the panel 480 px wide. `LATHE_FRAMES=1` times each frame's
layout and paint of the whole window on the CPU; the GPU work is not counted. Load average 4.8 to 6.5
from alex-31's builds.

    LATHE_FRAMES=1 target/release/lathe /tmp/qa-long     # then open the session and scroll

| Overdraw | Frame median | p95 | Worst | Over 8.3 ms | Layout median |
| --- | --- | --- | --- | --- | --- |
| 600 px (first try) | 5.6 to 6.0 ms | 7.5 to 8.9 ms | 9.7 to 19.8 ms | 4 to 22 of 300 | 4.1 ms |
| 160 px (shipped) | 5.8 to 6.0 ms | 7.6 to 7.8 ms | 8.4 to 19.2 ms | 2 to 4 of 300 | 4.0 to 4.3 ms |

- The target was no frame over 8.3 ms. It is nearly met, not met: about 1% of frames go over, and
  the worst ones reach 11 to 19 ms. Most of a frame is layout of the whole window (4 ms), not the
  session's rows. The HP renders in software, so a Mac should be faster; alex-9c can run the same line
  there.
- Folding the 2,000 events and the first draw: 0.9 ms and 0.8 ms in the unit bench
  (`cargo test --release -p lathe-app -- --ignored --nocapture a_long_session`), which shapes no text.
- A stream costs one repaint a frame: the queue wakes the session once per batch.
## The pull request view
The gallery's "Pull request view" story on a large pull request: 300 changed files, 5,000 comments (1,000
threads of five, spread over the files), real git and an in-memory forge, release, on the HP. The HP draws
in software (about 80 ms a frame whatever the code does), so the numbers to read are our own layout,
prepaint and paint. The run opens the pull request, waits for the first diff, then draws 300 frames: the
rail scrolls every frame and every tenth frame goes to the next file.
    GALLERY_STORY="Pull request view" GALLERY_SCROLL=1 PRV_FILES=300 PRV_COMMENTS=5000 target/release/beui-gallery
| Case (300 frames) | Target | Layout and prepaint, median | p95 | Paint, median | Layout nodes, most |
| --- | --- | --- | --- | --- | --- |
| 300 files, 5,000 comments | layout under 8 ms | 3.9 ms | 8.5 to 9.0 ms | 1.6 ms | 220 |
| The frames that go to the next file | | 5.1 to 5.4 ms | 10.8 to 18.9 ms | | |
The load average was 6 to 20 during these runs. The "Tasks" story ran in the same session as a control
(5,000 tasks scrolled): layout median 3.5 ms, p95 8.8 ms, 20 of 298 frames over 8.33 ms; the pull request
view had 15 to 20 of 270. So the view is as heavy as the board and the list, and the frames over budget
come from load, as `docs/performance.md` says of them above. Not measured on the Mac.
Before this, the rail listed every thread as a row and drew the same run at 40 ms of layout and 11,296
nodes. Now it lists 20 threads and 20 remarks and counts all of them ("Show more" adds 20), a thread of
more than three comments shows its first and its last in the diff, and the model builds only the page
(0.04 ms for 5,000 comments; it took 3 to 5 ms when it built all). What is left is mostly the comments in
the diff: with the threads and the rail both off, layout is 0.9 ms; the rail costs about 2 ms and the
threads under the rows about 2 ms when every file has three.
Open to first paint from the local cache, in the view's own timeline (`PullView::timeline`, printed at
the end of the run) and in the ignored test `cargo test --release -p lathe-pr-view perf -- --ignored
--nocapture`, which makes the same pull request on disk:
| Step, from the view being made | Test (release, HP) | Story run |
| --- | --- | --- |
| The saved read of 5,000 comments is on screen | 6 ms | 75 to 220 ms under load |
| git has answered (cache, 300 files, commits, base) | 107 ms | 270 to 690 ms under load |
| The first diff is read | 155 ms | 270 to 890 ms under load |
git alone, one call at a time, median: prepare 14 to 37 ms, the 300-file list 11 to 19 ms, the commits 4 to 5
ms, the two blobs of one file 4 to 5 ms, and building the diff 0.01 ms. They add up to 35 to 65 ms and
the target of 200 ms holds in the test; the story run, on a machine at load average 20 to 40, took up to
four times as long for the same work. The window shows the saved read (header, checks, conversation, tree
from the forge's file list) before git answers.

## Review in the app

A real turn of 200 files (claude ran `sed -i 1s/value/amount/ src/*.rs` over 199 small Rust files and
one of 20,000 lines), then the review, in the release app under Xvfb at 1440×900. `LATHE_TIMINGS=1`
prints each open and each whole-file decision: the pane's own work, how long after the press the frame
that shows it began, and that frame's layout and paint (`LATHE_FRAMES=1`). The load average was 23 to 27
from other builds on the HP.

    LATHE_TIMINGS=1 LATHE_FRAMES=1 target/release/lathe ~/qa/big    # then Review, Escape, and ⌃⇧↵

| Case | Target | Median | p95 or worst | Runs | Result |
| --- | --- | --- | --- | --- | --- |
| Open, first file 20,000 lines: press to the frame drawn | < 100 ms | 97.0 ms | 154.7 ms | 11 | Median at the target, p95 over |
| The same: the pane's work | | 15.6 ms | 33.0 ms | 11 | |
| The same: the first frame's layout and paint | | 61.9 ms | 112.4 ms | 11 | |
| Open, first file 5 lines: press to the frame drawn | < 100 ms | 56 ms | 111 ms (the first open) | 8 | Passes but for the first |
| Accept a whole file: the frame that shows it | one frame (8.3 ms) | 5.7 ms | 9.1 ms (worst 9.8) | 20 | 18 of 20 in a frame |
| The same: the pane's work | | 0.9 ms | 5.1 ms | 20 | |

- About 30 to 45 ms of each press-to-frame time is the wait before the frame begins, which a light
  action (1 ms of work) shows too: the frame clock under Xvfb, and the load.
- The open's cost is the 20,000-line editor: 15 ms to build its state, and about 50 ms in its first
  layout. That is the editor's own, as it is for a tab; it is the next thing to measure on the Mac and
  to cut.
- Before gpui-component patch 3, each new editor compiled the Rust highlight queries (about 50 ms) on
  the UI thread in its first frame: accepting a file, which opens the next one in a new editor, took a
  30 to 65 ms frame (median 51 ms). With the queries compiled once per process, 5.7 ms. The first open of
  a language in a process still pays it once (the 111 ms above).
