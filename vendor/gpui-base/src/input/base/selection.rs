use crate::input::InputModeKind;
use std::ops::Range;

use gpui::{Context, Window};
use ropey::Rope;
use sum_tree::Bias;

use super::{InputBaseState, RopeExt as _};
use crate::text_boundary::word_range_from_chars;

/// Unique identifier for a cursor/selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub(super) struct CursorId(usize);

impl CursorId {
    pub(super) fn new(id: usize) -> Self {
        Self(id)
    }
}

impl<M: InputModeKind> InputBaseState<M> {
    /// Select the word at the given offset on double-click.
    ///
    /// The offset is the UTF-8 offset.
    pub(super) fn select_word(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
        // A masked value renders as one unbroken run of mask characters, so it
        // has no word boundaries to select by. Take all of it instead, rather
        // than let the selection highlight reveal where the words are.
        let range = if self.masked {
            0..self.text.len()
        } else {
            let Some(range) = TextSelector::word_range(&self.text, offset) else {
                return;
            };
            range
        };

        self.undo_manager.break_transaction_coalescing();
        self.selections.remove_all_but_active();
        self.set_selection(range.start, range.end);
        self.selected_word_range = Some(*self.active_selection());
        cx.notify()
    }

    /// Select the line at the given offset on triple-click.
    ///
    /// The offset is the UTF-8 offset.
    pub(super) fn select_line(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
        let range = TextSelector::line_range(&self.text, offset);
        self.undo_manager.break_transaction_coalescing();
        self.selections.remove_all_but_active();
        self.set_selection(range.start, range.end);
        self.selected_word_range = None;
        cx.notify()
    }
}

struct TextSelector;
impl TextSelector {
    /// Select a line in the given text at the specified offset.
    ///
    /// The offset is the UTF-8 offset.
    ///
    /// Returns the start and end offsets of the selected line.
    pub(crate) fn line_range(text: &Rope, offset: usize) -> Range<usize> {
        let offset = text.clip_offset(offset, Bias::Left);
        let row = text.offset_to_point(offset).row;
        let start = text.line_start_offset(row);
        let end = text.line_end_offset(row);

        start..end
    }

    /// Select a word in the given text at the specified offset.
    ///
    /// The offset is the UTF-8 offset.
    ///
    /// Returns the start and end offsets of the selected word.
    pub(crate) fn word_range(text: &Rope, offset: usize) -> Option<Range<usize>> {
        let offset = text.clip_offset(offset, Bias::Left);
        let Some(char) = text.char_at(offset) else {
            return None;
        };

        let end = offset + char.len_utf8();
        let prev_chars = text.chars_at(offset).reversed().take(128);
        let next_chars = text.chars_at(end).take(128);
        Some(word_range_from_chars(offset, char, prev_chars, next_chars))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn test_word_range() {
        use indoc::indoc;

        let rope = Rope::from(indoc! {
            r#"
            test text:
            abcde 中文🎉 test
            hello[()]
            test_connector ____
            Rope
            rök
            grande île
            "#
        });

        let tests = vec![
            (0, 0, Some("test")),
            (0, 4, Some(" ")),
            (1, 0, Some("abcde")),
            (1, 4, Some("abcde")),
            (1, 5, Some(" ")),
            (1, 6, Some("中")),
            (1, 9, Some("文")),
            (1, 13, Some("🎉")),
            (1, 20, Some("test")),
            (2, 5, Some("[")),
            (2, 6, Some("(")),
            (2, 7, Some(")")),
            (2, 8, Some("]")),
            (3, 5, Some("test_connector")),
            (3, 14, Some(" ")),
            (3, 16, Some("____")),
            (4, 0, Some("Rope")),
            (5, 0, Some("rök")),
            (6, 8, Some("île")),
        ];

        for (line, column, expected) in tests {
            let line_start_offset = rope.line_start_offset(line);
            let offset = line_start_offset + column;
            let range = TextSelector::word_range(&rope, offset);

            let actual = range.map(|r| rope.slice(r).to_string());
            let expect = expected.map(|s| s.to_string());
            assert_eq!(actual, expect, "line {}, column {}", line, column);
        }
    }

    #[test]
    fn test_line_range() {
        let rope = Rope::from("first line\nsecond line\nthird");
        let tests = vec![
            (0, 0, "first line"),
            (0, 5, "first line"),
            (1, 3, "second line"),
            (2, 1, "third"),
        ];

        for (line, column, expected) in tests {
            let line_start_offset = rope.line_start_offset(line);
            let offset = line_start_offset + column;
            let range = TextSelector::line_range(&rope, offset);

            let actual = rope.slice(range).to_string();
            assert_eq!(actual, expected, "line {}, column {}", line, column);
        }
    }
}

/// lathe patch: the next place `query` occurs after `from`, wrapping to the top, that no selection
/// in `taken` already covers. `whole_word` skips a match with a word character on either side, as
/// Zed does when the search began from a bare caret.
pub(super) fn next_occurrence(
    text: &str,
    query: &str,
    from: usize,
    whole_word: bool,
    taken: &[Range<usize>],
) -> Option<Range<usize>> {
    if query.is_empty() {
        return None;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let fits = |start: usize| {
        let end = start + query.len();
        let clear_before = text[..start].chars().next_back().is_none_or(|c| !is_word(c));
        let clear_after = text[end..].chars().next().is_none_or(|c| !is_word(c));
        let free = !taken.iter().any(|t| t.start == start && t.end == end);
        free && (!whole_word || (clear_before && clear_after))
    };
    let after = text[from..].match_indices(query).map(|(i, _)| from + i);
    let before = text[..from].match_indices(query).map(|(i, _)| i);
    after.chain(before).find(|&start| fits(start)).map(|start| start..start + query.len())
}

/// lathe patch: the word a caret at `offset` stands in or just after, as Zed picks it. A caret after
/// the last letter of a word takes that word, not the space or newline that follows.
pub(super) fn word_at_caret(text: &str, offset: usize) -> Option<Range<usize>> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..offset].char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map_or(offset, |(i, _)| i);
    let end = offset + text[offset..].chars().take_while(|c| is_word(*c)).map(char::len_utf8).sum::<usize>();
    (start < end).then_some(start..end)
}

impl<M: InputModeKind> InputBaseState<M> {
    /// lathe patch: Zed's select next occurrence. From a bare caret it selects the word there; from
    /// a selection it adds one more selection at the next match of its text.
    pub(super) fn select_next_occurrence(
        &mut self,
        _: &super::SelectNextOccurrence,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_multi_line() {
            return;
        }
        self.undo_manager.break_transaction_coalescing();
        let newest = *self.selections.iter().last().expect("there is always a selection");
        if newest.is_empty() {
            let Some(word) = word_at_caret(&self.text.to_string(), newest.cursor_offset()) else {
                return;
            };
            let mut all: Vec<_> = self.selections.iter().copied().collect();
            let last = all.last_mut().expect("there is always a selection");
            *last = super::cursor::CursorSelection::new(last.id, word.start, word.end);
            self.selections.replace_all(all);
        } else {
            let text = self.text.to_string();
            let query = &text[newest.start..newest.end];
            let whole_word = word_at_caret(&text, newest.start) == Some(newest.start..newest.end);
            let taken: Vec<_> = self.selections.iter().map(|s| s.start..s.end).collect();
            let Some(found) = next_occurrence(&text, query, newest.end, whole_word, &taken) else {
                return;
            };
            let id = self.selections.generate_id();
            self.selections.add(super::cursor::CursorSelection::new(id, found.start, found.end));
        }
        let head = self.selections.iter().last().expect("there is always a selection").end;
        self.scroll_to(head, None, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod next_occurrence_tests {
    use super::next_occurrence;

    #[test]
    fn a_match_after_the_selection_comes_first() {
        assert_eq!(next_occurrence("a b a b a", "a", 1, false, &[0..1]), Some(4..5));
    }

    #[test]
    fn the_search_wraps_to_the_top() {
        assert_eq!(next_occurrence("x a x", "x", 5, false, &[4..5]), Some(0..1));
    }

    #[test]
    fn a_taken_match_is_skipped_and_none_is_left_at_the_end() {
        assert_eq!(next_occurrence("x x", "x", 1, false, &[0..1, 2..3]), None);
    }

    #[test]
    fn a_whole_word_search_skips_a_match_inside_a_longer_word() {
        assert_eq!(next_occurrence("ab abc ab", "ab", 2, true, &[0..2]), Some(7..9));
        assert_eq!(next_occurrence("ab abc ab", "ab", 2, false, &[0..2]), Some(3..5));
    }

    #[test]
    fn a_caret_after_a_word_takes_that_word() {
        use super::word_at_caret;
        assert_eq!(word_at_caret("ab cd\n", 5), Some(3..5), "at the end, before a newline");
        assert_eq!(word_at_caret("ab cd", 4), Some(3..5), "inside");
        assert_eq!(word_at_caret("ab cd", 3), Some(3..5), "at the start");
        assert_eq!(word_at_caret("ab  cd", 3), None, "between two spaces there is no word");
        assert_eq!(word_at_caret("été x", 0), Some(0..5), "letters beyond ASCII count");
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        assert_eq!(next_occurrence("abc", "", 0, false, &[]), None);
    }
}
