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
