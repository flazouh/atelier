# gpui-base 0.6.6, patched for lathe

This is gpui-base 0.6.6 from crates.io (github.com/longbridge/gpui-kit), unchanged except for the
patches below. Each one is small and carries its own test, so it can go upstream and be dropped here.
To upgrade: copy the new release over this directory, then re-apply each patch that upstream lacks.

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
