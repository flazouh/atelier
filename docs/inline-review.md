# The inline review

The agent's edits shown inside the file the user is editing, the way Cursor does it, rather than in a
separate diff pane. This replaces the `read_only` lock that Stage A needs today.

Written 2026-09-27 at commit `a201728`. Every API named here was read from
`gpui-base` 0.6.6 source, and every claim below says where.

## Why the shape changed

Stage A (`hunk.rs`) draws its own rows, so it can animate a row's height to zero. A real editor buffer
cannot: `calculate_visible_range` divides pixels by one `line_height`, and there is no height map, so
every row is the same height and no row can shrink. `TextDecoration` cannot help either, because its
background quad runs from the first glyph to the last glyph's advance
(`gpui-pre-0.3.6/src/text_system/line.rs:845`). A short line would get a ragged stub and a blank line
no band at all.

So the inline review is built from four public parts instead:

| Need | Public API | Source |
|---|---|---|
| A full-width wash on one row | `text_bounds()` for x and width, `range_to_bounds()` for y, `line_height()` for height | `state.rs:541`, `:3413`, `:2820` |
| Tint the sign and the removed text | `create_decorations_collection` then `.set()`, with `HighlightStyle` | `editor/decorations.rs:238`, `:45` |
| Place the per-hunk bar on its rows | `range_to_bounds(range)` | `state.rs:3413` |
| Accept or reject one hunk | `set_selected_range(range)` then `replace(text)` | `state.rs:2846`, `:977` |
| Keep the user's undo history | `replace` records `EditIntent::Atomic` | `state.rs:981` |
| Hold the caret and the scroll | `selected_range`, `scroll_offset`, `set_scroll_offset` | `state.rs:2831`, `:2807`, `:2814` |
| Decorate only what is on screen | `visible_row_range()` | `state.rs:2802` |

`replace_text_in_ranges`, the one atomic multi-range path, is `pub(crate)` (`state.rs:3457`). So a
multi-hunk accept is a loop of single-range `replace` calls, applied from the last hunk to the first so
that no earlier edit shifts a later range.

## Both sides live in the buffer

The buffer holds the removed lines and the added lines at once, as real text, the way a conflict marker
does. Nothing is hidden and nothing is virtual.

- Accept: select the removed rows, replace with nothing. The added rows stay.
- Reject: select the added rows, replace with nothing. The removed rows stay.
- Either way the user can undo, because `replace` records the edit.
- The user can type anywhere at any time. There is no lock, and no product has one.

## The resolve, honestly

A row cannot shrink, so the resolve is a fade and then a cut, not a collapse:

1. The hunk's wash fades to nothing over `duration::RESOLVE` (260ms) on `ease::MORPH`.
2. The surviving side's wash fades out with it, so the code settles into plain text.
3. At the end, the buffer edit runs, and the resolved rows go at once.

The fade leads the cut, so the eye has already left the rows before they vanish. Under Reduce Motion the
edit runs at once with no fade. Write this down rather than claim a collapse we cannot draw.

## What to test first, with no window

- `plan_edits(hunks, decision)`: the byte ranges to replace, ordered last to first.
- `surviving_side(hunk, decision)`: which rows remain.
- `band_rows(hunk, visible)`: the rows needing a wash, clipped to the visible range.
- `hold_caret(before, edits)`: where the caret lands after the edits, so it never jumps to 0,0.

## Checks

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.
- A gallery story "Inline review", captured on `hp-agent` in both themes.
- Sample the band pixel and prove it reaches the right edge of the text area on a one-character line.
- Accept one hunk of three, then undo, and prove the text and the caret both come back.
- Type inside a hunk, then accept it, and prove the typing survives.
