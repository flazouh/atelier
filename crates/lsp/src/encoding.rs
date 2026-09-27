//! Where a column is, in the unit a server counts.
//!
//! gpui-base counts a column in characters. LSP servers count in UTF-16 units unless the client and
//! the server agree on another encoding. The worker offers UTF-32 (characters) and UTF-16, and
//! converts at its boundary with these functions, so nothing outside it deals with encodings.

use lsp_types::{Position, PositionEncodingKind, Range};

/// The unit a server counts columns in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16,
    Utf32,
}

impl Encoding {
    /// The encoding a server chose. A server that names none uses UTF-16, as LSP 3.17 says.
    pub fn negotiated(kind: Option<&PositionEncodingKind>) -> Self {
        match kind.map(|k| k.as_str()) {
            Some("utf-8") => Self::Utf8,
            Some("utf-32") => Self::Utf32,
            _ => Self::Utf16,
        }
    }

    fn width(self, c: char) -> u32 {
        match self {
            Self::Utf8 => c.len_utf8() as u32,
            Self::Utf16 => c.len_utf16() as u32,
            Self::Utf32 => 1,
        }
    }
}

fn line(text: &str, row: u32) -> &str {
    text.split('\n').nth(row as usize).unwrap_or("")
}

/// A character position in `text`, as the server counts it.
pub fn to_server(text: &str, position: Position, encoding: Encoding) -> Position {
    let units = line(text, position.line).chars().take(position.character as usize).map(|c| encoding.width(c)).sum();
    Position { line: position.line, character: units }
}

/// A server position in `text`, as a character position. A column inside a character, or past the
/// end of the line, lands on the nearest character boundary before it.
pub fn from_server(text: &str, position: Position, encoding: Encoding) -> Position {
    let mut units = 0;
    let mut characters = 0;
    for c in line(text, position.line).chars() {
        units += encoding.width(c);
        if units > position.character {
            break;
        }
        characters += 1;
    }
    Position { line: position.line, character: characters }
}

/// [`from_server`] for both ends of a range.
pub fn range_from_server(text: &str, range: Range, encoding: Encoding) -> Range {
    Range { start: from_server(text, range.start, encoding), end: from_server(text, range.end, encoding) }
}

#[cfg(test)]
mod tests;
