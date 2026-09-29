# gpui-base 0.6.6, patched for lathe

This is gpui-base 0.6.6 from crates.io (github.com/longbridge/gpui-kit), unchanged except for the
patches below. Each one is small and carries its own test, so it can go upstream and be dropped here.
To upgrade: copy the new release over this directory, then re-apply each patch that upstream lacks.
The workspace excludes this crate, so `tools/check.sh` runs its tests, the patch tests among them.

## 1. Shift+Up/Down keep the goal column

`select_up` and `select_down` in `src/input/base/state.rs` jumped to the edge of the next line
(`previous_boundary` of the line start, `next_boundary` of the line end) instead of the carets
## 1. Shift+Up/Down keep the goal column

`select_up` and `select_down` in `src/input/base/state.rs` jumped to the edge of the next line
(`previous_boundary` of the line start, `next_boundary` of the line end) instead of the caret's
column. They now share `select_vertical`, which uses `vertical_target` and the selection's
`column_anchor`, as `move_vertical` does for plain Up and Down. Past the first or last row the head
goes to the very start or end of the text, as in Zed.
Test: `test_shift_up_and_down_keep_the_goal_column` (fails on 0.6.6 with `(8..13, 8)`).

## 2. Select next occurrence (cmd-d, ctrl-d on Linux and Windows)

gpui-base had multi-cursor but no way to add a selection at the next match. A new action,
`SelectNextOccurrence`, works as Zed's: from a bare caret it selects the word the caret stands in or
just after; from a selection it adds a selection at the next match of its text, wrapping to the top
and skipping matches already selected. A selection that is exactly a word matches whole words only.
Code: `select_next_occurrence`, `next_occurrence` and `word_at_caret` in `src/input/base/selection.rs`,
the binding and the listener in `src/input/base/state.rs`.
Tests: `test_select_next_occurrence_adds_each_match_in_turn` and `next_occurrence_tests`.

## 3. A pinned editor style

gpui-component's `Input` calls `set_editor_style` with its theme's colours on every render, after
the owner has set its own, so an owner's style never reached the screen: lathe's selection, current
line, fill and diagnostic colours were all replaced. `pin_editor_style(Some(style))` makes every later
`set_editor_style` apply the pinned style instead; `None` unpins.
Code: `pinned_editor_style`, `set_editor_style`, `pin_editor_style` in `src/input/base/state.rs`.
Test: `test_a_pinned_editor_style_outlasts_a_theme_style`.

## 4. Vertical moves at the edges, and the Cmd underline on release

- Up on the first row goes to the very start of the text, and Down on the last row to the very end,
  as in Zed. The goal column survives the trip. (`move_vertical` in `src/input/base/movement.rs`.)
- A caret placed by `set_selected_range` had no column anchor, so the next Up or Down landed in
  column 0. It now uses its own column. (Same place.)
- Letting go of Cmd drops the definition underline instead of leaving it until the pointer moves.
  (`clear_hover_definition` in `src/input/base/kind.rs` and `src/input/editor/mod.rs`, called from
  the `on_modifiers_changed` listener in `src/input/base/state.rs`.)
Test: `test_up_on_the_first_row_and_down_on_the_last_reach_the_ends`. The underline was checked by
hand under Xvfb.

## 5. A `⋯` chip after a folded line

A fold hid its rows and flipped the gutter chevron, but the text showed nothing. As in Zed, a folded
line now ends in a small `⋯` chip, and a click on it unfolds the rows. Code: `FoldIconLayout::
placeholders`, laid out in `layout_fold_icons` and painted in `paint_fold_icons`, in
`src/input/base/element.rs`. Checked by hand under Xvfb: fold, chip shown, click, rows back.

## 6. F12 goes to definition from the caret

`GoToDefinition` only reused the location a Cmd-hover had cached, so with the pointer elsewhere it
did nothing, and nothing bound it. With no cached answer for the caret it now asks the
`DefinitionProvider` at the caret and jumps through the same `go_to_definition` path a Cmd-click
takes, `show_document` included. F12 is bound to it.
Code: `on_action_go_to_definition` in `src/input/editor/lsp/definitions.rs`, the binding in
`src/input/base/state.rs`. Checked by hand under Xvfb.

## 7. Every selection, readable and settable

`selected_ranges()` returns every selection in text order, and `set_selected_ranges(&[..])` replaces
them all, so an owner's multi-cursor command can put every cursor back. Before, only the active
selection was public, and `set_selected_range` dropped the others.
Test: `test_selected_ranges_round_trip`.

Patches 1 and 4 share `vertical_target_or_edge` in `src/input/base/movement.rs`.

## 8. Row washes and row widgets painted by the editor

An owner that drew its own row bands had to guess the editor's top padding, row height, wrapping and
scroll. The inline review guessed `row * 20 + scroll`, and on macOS, where gpui-component pads the
editor, every band and bar sat about 8pt above its row. The editor now does it from its own layout:
`set_row_backgrounds` washes whole rows under the text and again over the gutter, with an optional
2px marker at the gutter's left edge, as Zed marks changed lines; `set_row_widgets` places an
element at a row's right end, centred on the row, laid out in the same frame as the text and clipped
to the editor. `LastLayout::row_rects` is the one place both read row geometry from.
Code: `src/input/base/layout.rs`, `src/input/base/state.rs`, `src/input/base/element.rs`.
Test: `test_row_rects_start_where_each_row_of_text_starts`. Checked on a Retina Mac.

## 9. The I-beam over the text only

The editor root asked for the I-beam over its whole area, so the gutter, the fold chevrons and any
control on top of the text showed it too. The text element now sets the I-beam (the crosshair with
Alt held) over the text area only and the arrow over the gutter, before it paints anything else, so
fold chevrons, the fold chip, row widgets and a Cmd-hovered link set their own pointer over them.
Fold chevrons show a hand. Code: `TextElement::paint` and `text_hitbox` in
`src/input/base/element.rs`; the root's `cursor_text` is gone from `src/input/base/state.rs`.
Checked under Xvfb by reading the X cursor image (XFixes): I-beam over text and blank text, arrow
over the gutter and the page, hand over buttons, chevrons, the fold chip and a Ctrl-hovered symbol.

## 10. Cmd pressed over a symbol shows the link at once

The definition underline was looked up only on a mouse move with Cmd held, so pressing Cmd with the
pointer already on a symbol showed nothing until the pointer moved. The `on_modifiers_changed`
listener in `src/input/base/state.rs` now runs the same lookup when Cmd goes down over the text.
Checked by hand under Xvfb.

## 11. Row covers and row gaps, for a hunk that closes

A buffer row cannot shrink, so a hunk that left the buffer made the rows below it jump up. Two owner
APIs let a review animate it. `set_row_covers` paints fills over whole rows after the text and again
after the line numbers, so a cover in the editor's background colour fades rows out, text and all.
`set_row_gaps` paints empty space above a row, which pushes that row and every row below it down;
shrinking it frame by frame slides those rows up. `LastLayout::gap_above` is the one place the gap is
read from: the text, glyph backgrounds, current line, line numbers, washes, covers, row widgets, indent
guides and carets all add it, and since patch 12 so do the pointer's hit test, the selection
highlight, and IME and touch positions. Only the scroll height leaves it out, which is fine for a gap
that lives 190ms.
Code: `src/input/base/layout.rs`, `src/input/base/state.rs`, `src/input/base/element.rs`,
`src/input/editor/indent.rs`.
Test: `test_a_row_gap_pushes_its_row_and_the_rows_below_down`.

## 12. Row blocks: an element in a gap below a row

A comment thread must sit under its row and push the rows below it down, for as long as it is open.
A row gap (patch 11) only moves what is painted, and a row widget sits on its row. `set_row_blocks`
places an owner's element in a gap below its row, as wide as the text and as tall as its content:
the editor lays each block out at the text's width in the frame it lays the text out, so the gap is
always exactly its height. The pointer's hit test, the selection highlight, the caret, IME and touch
positions, the fold chevrons and the fold chip, and the scroll height all add it (all but the last
now add patch 11's gaps too), and the
visible range reaches up far enough to lay out rows the blocks push into view. Each block blocks the
pointer and is placed after the editor's own hitboxes, so a press in it never moves the caret. A block
on a folded row gets no gap. Blocks on the same row stack in the order the owner gives them.
Code: `RowBlock` and `set_row_blocks` in `src/input/base/state.rs`, `measure_row_blocks` and
`place_row_blocks` in `src/input/base/element.rs`.
Test: `test_a_row_block_opens_a_gap_below_its_row`.

## 13. A gutter widget on the row under the pointer

A review opens a comment from a "+" in the gutter of the row the pointer is over, as GitHub does.
`set_gutter_widget` takes a function of the row; the editor tracks the row under the pointer
(`hovered_row`) from a window-level mouse-move listener gated on its own hitbox, redraws when it
changes, and draws the widget at the gutter's left edge, centred on that row. The row clears when
the pointer leaves the editor or moves over a row block, which blocks the editor's hitbox.
Code: `GutterWidget`, `set_gutter_widget`, `hovered_row` and `set_hovered_row` in
`src/input/base/state.rs`; `layout_gutter_widget` and the listener in `TextElement::paint` in
`src/input/base/element.rs`.
Test: `test_the_gutter_widget_follows_the_row_under_the_pointer`.

## 14. A read-only input says so in its key context

An input's key context was `Input` whether it could be typed in or not, so an owner that lets bare
letters through only when nobody is typing (lathe's review keys: `s`, `w`, `u`) could not tell a
read-only editor, such as a pull request's diff, from one being typed in. A read-only input's context
is now `Input readonly`; bindings on `Input` still match it.
Test: `test_a_read_only_input_says_so_in_its_key_context`.

## 15. The offset under the pointer

`offset_at_pointer(window)` is the text offset under the pointer when the pointer is over the text,
and `None` elsewhere, so an owner can act on "the name under the pointer", as lathe's `u` (Uses) does
in a pull request's diff. It is `index_for_mouse_position` behind the bounds check the Cmd underline
already makes.
Test: `test_the_offset_under_the_pointer`.

## 16. The syntax styles, readable

`syntax_styles(range, resolver)` returns the styles the editor's highlighter holds for a range now,
as the editor paints them, so a test can compare them with a fresh parse after an edit that took
the background path. lathe's `crates/beui/src/code_editor/tests.rs` (module `background`) does.
It borrows the highlighter the editor already keeps; nothing else changes.

## 17. A caret set before the first layout is revealed

`scroll_to` needs the last layout, so a caret moved in a buffer that has never been painted, such
as a file just opened at a definition, moved without scrolling and the view stayed at the top. The
state now keeps that offset (`reveal_after_layout`), and the first paint reveals it.
Test: `test_a_caret_set_before_the_first_layout_is_revealed`.
