use lsp_types::{Position, Range};

use super::types::Encoding;

pub(super) fn line(text: &str, row: u32) -> &str {
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
