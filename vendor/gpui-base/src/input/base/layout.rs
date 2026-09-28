use std::{ops::Range, rc::Rc};

use gpui::{Bounds, Half, Pixels, ShapedLine, TextAlign, px};

use super::{WrappingIndent, display_map::LineLayout};

#[derive(Clone, Default)]
pub(crate) struct WhitespaceIndicators {
    pub(crate) space: ShapedLine,
    pub(crate) tab: ShapedLine,
}

#[derive(Clone)]
pub(super) struct LastLayout {
    pub(super) visible_range: Range<usize>,
    pub(super) visible_buffer_lines: Vec<usize>,
    pub(super) visible_line_byte_offsets: Vec<usize>,
    pub(super) visible_top: Pixels,
    pub(super) visible_range_offset: Range<usize>,
    pub(super) lines: Rc<Vec<LineLayout>>,
    pub(super) line_height: Pixels,
    pub(super) wrap_width: Option<Pixels>,
    pub(super) wrapping_indent: WrappingIndent,
    pub(super) line_number_width: Pixels,
    /// Width of one space in the editor font.
    ///
    /// Past the end of a line there are no glyphs to hit-test against, so this is the
    /// step used to measure how far past the end a pointer sits.
    pub(super) space_width: Pixels,
    pub(super) cursor_bounds: Option<Bounds<Pixels>>,
    pub(super) text_align: TextAlign,
    pub(super) content_width: Pixels,
    /// lathe patch: the owner's row gaps as `(row, height)`. See `InputBaseState::set_row_gaps`.
    pub(super) row_gaps: Vec<(usize, Pixels)>,
}

impl LastLayout {
    /// lathe patch: every visible buffer row as `(row, top, height)`, the top measured from the text
    /// origin as the paint pass measures it (`visible_top` included). `extra` adds height after one
    /// row, which is how inline-completion ghost lines push the rows below them down.
    pub(super) fn row_rects(&self, extra: Option<(usize, Pixels)>) -> Vec<(usize, Pixels, Pixels)> {
        let mut top = self.visible_top;
        let mut rects = Vec::with_capacity(self.lines.len());
        for (line, &row) in self.lines.iter().zip(self.visible_buffer_lines.iter()) {
            let height = self.line_height * line.wrapped_lines.len() as f32;
            rects.push((row, top + self.gap_above(row), height));
            top += height;
            if let Some((after, more)) = extra
                && after == row
            {
                top += more;
            }
        }
        rects
    }

    /// lathe patch: how far the row gaps push buffer row `row` down: every gap at or above it.
    pub(super) fn gap_above(&self, row: usize) -> Pixels {
        self.row_gaps.iter().filter(|(at, _)| *at <= row).fold(px(0.), |sum, (_, gap)| sum + *gap)
    }

    pub(crate) fn line(&self, row: usize) -> Option<&LineLayout> {
        let pos = self.visible_buffer_lines.binary_search(&row).ok()?;
        self.lines.get(pos)
    }

    pub(super) fn alignment_offset(&self, line_width: Pixels) -> Pixels {
        match self.text_align {
            TextAlign::Left => px(0.),
            TextAlign::Center => (self.content_width - line_width).half().max(px(0.)),
            TextAlign::Right => (self.content_width - line_width).max(px(0.)),
        }
    }
}
