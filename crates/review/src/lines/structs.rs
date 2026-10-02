use imara_diff::TokenSource;

/// A text split into rows without their line ends, and whether it ended with one. An empty text has no
/// rows; a text that is only a line end has one empty row.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rows<'a> {
    pub rows: Vec<&'a str>,
    pub final_newline: bool,
}

impl<'a> Rows<'a> {
    pub fn of(text: &'a str) -> Self {
        if text.is_empty() {
            return Self { rows: Vec::new(), final_newline: false };
        }
        let final_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        Self { rows: body.split('\n').collect(), final_newline }
    }
}

/// The rows of a text as tokens: two rows are the same token when their text is the same.
#[derive(Clone, Copy)]
pub(crate) struct RowTokens<'a>(pub &'a [&'a str]);

impl<'a> TokenSource for RowTokens<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Copied<std::slice::Iter<'a, &'a str>>;

    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().copied()
    }

    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}
