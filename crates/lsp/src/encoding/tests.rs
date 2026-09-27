use super::*;

fn at(line: u32, character: u32) -> Position {
    Position { line, character }
}

/// "a😀b" on line 1: the emoji is one character, two UTF-16 units and four UTF-8 bytes.
const TEXT: &str = "first\na😀b = é";

#[test]
fn a_server_that_names_no_encoding_counts_in_utf16() {
    assert_eq!(Encoding::negotiated(None), Encoding::Utf16);
    assert_eq!(Encoding::negotiated(Some(&PositionEncodingKind::UTF32)), Encoding::Utf32);
    assert_eq!(Encoding::negotiated(Some(&PositionEncodingKind::UTF8)), Encoding::Utf8);
}

#[test]
fn a_column_after_an_emoji_differs_by_encoding() {
    // Character 2 is the `b` after the emoji.
    assert_eq!(to_server(TEXT, at(1, 2), Encoding::Utf32), at(1, 2));
    assert_eq!(to_server(TEXT, at(1, 2), Encoding::Utf16), at(1, 3));
    assert_eq!(to_server(TEXT, at(1, 2), Encoding::Utf8), at(1, 5));
}

#[test]
fn a_server_column_comes_back_to_the_same_character() {
    for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
        for character in 0..=7 {
            let server = to_server(TEXT, at(1, character), encoding);
            assert_eq!(from_server(TEXT, server, encoding), at(1, character), "{encoding:?} {character}");
        }
    }
}

#[test]
fn a_column_inside_a_character_lands_before_it() {
    // UTF-16 unit 2 is the second half of the emoji.
    assert_eq!(from_server(TEXT, at(1, 2), Encoding::Utf16), at(1, 1));
}

#[test]
fn a_column_past_the_end_of_the_line_stops_at_its_end() {
    assert_eq!(from_server(TEXT, at(0, 99), Encoding::Utf16), at(0, 5));
    assert_eq!(to_server(TEXT, at(0, 99), Encoding::Utf16), at(0, 5));
}
