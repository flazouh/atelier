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
that fell under the cap. `compute_injection_layers` shares its match loop (`find_injections`) and
now returns `InjectionLayers`, which carries what the next edit needs; layers sort by start, end and
language, so equal starts come out the same on every pass. Two `#[doc(hidden)]` accessors,
`injection_layer_ranges` and `injections_edited`, let the test compare highlighters.

Test: `tests/injection_edits.rs`. After each of 300 seeded random edits to Rust with macros, Markdown
with fences and inline code, and HTML with script and style, and 40 to a Rust file past the layer
cap, the layers and styles equal a highlighter that parsed the same text from scratch, and at least
three edits in four took the in-place path. `many_seeds` (ignored) runs 12 more seeds of 400 edits.
