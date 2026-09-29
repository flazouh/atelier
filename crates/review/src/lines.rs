//! A text as rows, and the rows as a token source for the diff.
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

/// Rows joined back into a text with its final line end as it was.
pub(crate) fn join(rows: impl IntoIterator<Item = impl AsRef<str>>, final_newline: bool) -> String {
    let mut out = String::new();
    let mut any = false;
    for row in rows {
        if any {
            out.push('\n');
        }
        out.push_str(row.as_ref());
        any = true;
    }
    if any && final_newline {
        out.push('\n');
    }
    out
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

#[cfg(test)]
mod tests;
