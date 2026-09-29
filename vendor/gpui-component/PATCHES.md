# gpui-component 0.6.6, patched for lathe

This is gpui-component 0.6.6 from crates.io (github.com/longbridge/gpui-kit), unchanged except for the
patches below. Each one is small and carries its own test, so it can go upstream and be dropped here.
To upgrade: copy the new release over this directory, then re-apply each patch that upstream lacks.
The workspace excludes this crate, so `tools/check.sh` runs its tests, the patch tests among them.

## 1. Injection layers follow the edit

`SyntaxHighlighter::update` rebuilt every injection layer after each parse: it ran the injections
query over the whole tree and parsed again each layer whose byte range had moved. It did this on the
UI thread at every keystroke, outside the editor's 2 ms parse budget. On a 10k-line Rust file with
macros, a keystroke took 49 ms; with no macros it still took 21 ms, for the whole-tree query.

`update` now calls `edit_injection_layers` when it has an edit. It finds where the text or the tree
changed (the edit, plus `old_tree.changed_ranges(&new_tree)`), then:

- keeps each single layer whose match does not touch that region, and moves its ranges and its tree
  by the edit. A match covers every node it captured, so editing a fence's language drops its layer;
- runs the injections query only over the region, a byte wider on each side, and parses what it
  finds, which covers a macro the edit creates or deletes;
- rebuilds each combined layer (Markdown's inline text) from its raw ranges, kept in
  `combined_ranges`, and parses it afresh: parsing on the edited old tree differs from a fresh
  parse once the included ranges change;
- keeps the first `MAX_NON_COMBINED_INJECTION_PARSES` layers in text order, as the full pass does.

It returns `false` and the full pass runs when that cannot be exact: no edit, layers left stale by a
timed-out parse or `edit_tree`, a combined injection at its range or byte cap, or a capped document
that fell under the cap. A layer whose own parse runs out of its budget (20ms by default) leaves the
set incomplete, so the next edit builds all of them again rather than keep the hole. `compute_injection_layers` shares its match loop (`find_injections`) and
now returns `InjectionLayers`, which carries what the next edit needs; layers sort by start, end and
language, so equal starts come out the same on every pass. The layer budget, a constant before, is a
per-highlighter setting. Four `#[doc(hidden)]` items let the tests compare highlighters and pin the
budget: `injection_layer_ranges`, `injections_edited`, `injections_complete` and
`set_injection_budget`.

Test: `tests/injection_edits.rs`. After each of 300 seeded random edits to Rust with macros, Markdown
with fences and inline code, and HTML with script and style, and 40 to a Rust file past the layer
cap, the layers and styles equal a highlighter that parsed the same text from scratch, and at least
three edits in four took the in-place path. Both sides parse with no budget, so no step depends on
the machine's speed. `a_layer_out_of_time_makes_the_next_edit_rebuild` sets the budget to zero, sees
the set marked incomplete, and sees the next edit rebuild and equal a fresh highlighter. `many_seeds` (ignored) runs 12 more seeds of 400 edits.

## 2. The editor parses in the background, always

The editor's adapter (`input_adapter.rs`) parsed on the UI thread with a 2 ms budget, and waited
150 ms before a background parse only when that failed or the text passed 256 KB, cancelling the
running parse at every keystroke. A keystroke on a 10k-line Rust file cost 4 to 7 ms on the UI
thread even after patch 1: tree-sitter's own incremental parse and `changed_ranges`.

Now, for every text size:

- The UI thread only applies the edit. `edit_tree` edits the tree and moves each injection layer
  (`InjectionLayer::follow`), so the old colours stand where their text went: a range after the edit
  moves by its length change, one the edit falls inside grows or shrinks, and text typed exactly at a
  range's start or end joins neither side (`follow_range`). A whole-text replace (`reset_tree`)
  drops the tree, and its rows draw plain until the parse lands.
- The parse runs on a background thread through `background_parse` (a snapshot of the edited tree,
  the text and the injection data) and `BackgroundParse::run`, with no time budget.
  `apply_parsed` takes the result only if it was for the text held now; one for an older text is
  dropped.
- `ParseQueue` keeps one parse running per editor: the keystrokes during it wait as one queued parse,
  which starts on the newest text when the running one ends. There is no debounce.
- A combined layer (Markdown's inline text) that an edit moved is not offered for reuse to the next
  full pass: parsing again on its edited tree does not match a fresh parse.
- The background parse updates the layers in place, as patch 1 does for one edit (`update_layers`).
  The layers already moved with each edit, so it needs only where the edits wrote: `edit_tree` keeps
  one span of new text for all edits since the layers were last whole (`moved_since_parse`), each
  earlier span moved by the later edit and joined with its own. The region to query again is that
  span and `changed_ranges` between the edited old tree and the new one. It falls back to a full
  pass when the layers were not whole before the edits, or for patch 1's own reasons.

The inline review's accept and reject, paste, undo and redo all reach the adapter as edits, so they
take the same path. `apply_background_tree` and the sync-parse constants are gone.

Test: `tests/background_parse.rs`. A table of range moves; the old colours moved by an edit before
any parse; for Rust, Markdown and HTML, after each of 60 seeded edits, a landed background parse
equals a synchronous one; and 50 keystrokes during a parse make two parses, of which only the last is
taken. `coalesced_edits_update_the_layers_in_place_and_equal_a_fresh_parse` makes 80 parses of one to six
random edits each for Rust, Markdown and HTML: each landed parse equals a fresh one, and at least
three in four update the layers in place (80 of 80 in each language when written).

## 3. A language's queries compile once per process

`SyntaxHighlighter::new` compiled the language's highlight, locals and injection queries each time:
about 50 ms for Rust on the HP, on the UI thread, in the first frame of every editor that opened and
after every `set_value`, which drops the editor's highlighter. A review that steps through files made
a new editor for each, so each step's frame took 30 to 65 ms.

`Compiled` holds what the queries give (the queries, the pattern indices, the capture indices), built
the first time any highlighter of a language asks and kept for the process, by the language's name and
a hash of its query sources, so a language registered again with new queries compiles them anew. The
queries are only read after they are built, so every highlighter of the language shares them through
an `Arc`, on any thread. The lock is held while a language compiles, so two threads that want it wait
for one build. Each highlighter keeps its own parser.

Test: `tests/shared_queries.rs`. A second Rust highlighter builds at least ten times faster than the
first, which compiled the queries, and colours a text the same.
