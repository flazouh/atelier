use super::{Rows, join};

#[test]
fn a_text_is_its_rows_and_whether_it_ended_with_a_line_end() {
    fn rows(text: &str) -> (Vec<&str>, bool) {
        let r = Rows::of(text);
        (r.rows, r.final_newline)
    }
    assert_eq!(rows(""), (vec![], false));
    assert_eq!(rows("\n"), (vec![""], true));
    assert_eq!(rows("a"), (vec!["a"], false));
    assert_eq!(rows("a\n"), (vec!["a"], true));
    assert_eq!(rows("a\nb"), (vec!["a", "b"], false));
    assert_eq!(rows("a\n\nb\n"), (vec!["a", "", "b"], true));
    assert_eq!(rows("\n\n"), (vec!["", ""], true));
    assert_eq!(rows("a\r\nb\r\n"), (vec!["a\r", "b\r"], true), "a carriage return stays with its row");
}

#[test]
fn rows_join_back_into_the_text_they_came_from() {
    for text in ["", "\n", "a", "a\n", "a\nb", "a\n\nb\n", "\n\n", "é\nü\n", "x\r\ny", "no end", "  indented\n\ttab\n"] {
        let rows = Rows::of(text);
        assert_eq!(join(&rows.rows, rows.final_newline), text, "{text:?}");
    }
}

#[test]
fn nothing_joined_is_empty_whatever_the_ending() {
    assert_eq!(join(Vec::<&str>::new(), true), "");
}
