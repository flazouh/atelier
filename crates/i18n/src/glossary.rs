//! What the translator is told before it writes. Looked up by the English text, so a key that says "Cursor" is
//! told to leave it alone, and a key that says "Review" is told how the app says it in each language.

use super::Locale;

/// A word the app uses on purpose.
pub struct Term {
    /// The English word or name, matched whole and with its case: "Cursor" is the editor, "cursor" is where you type.
    pub source: &'static str,
    /// Written as it is in every language: a name.
    pub keep: bool,
    /// The approved translation per locale; a locale not listed is left to the translator.
    pub fixed: &'static [(Locale, &'static str)],
}

/// The names that are never translated. Product terms with an approved word per language are added as they come up.
pub const TERMS: &[Term] = &[
    Term { source: "Atelier", keep: true, fixed: &[] },
    Term { source: "Claude", keep: true, fixed: &[] },
    Term { source: "Claude Code", keep: true, fixed: &[] },
    Term { source: "Cursor", keep: true, fixed: &[] },
    Term { source: "Codex", keep: true, fixed: &[] },
    Term { source: "Git", keep: true, fixed: &[] },
    Term { source: "GitHub", keep: true, fixed: &[] },
    Term { source: "OpenRouter", keep: true, fixed: &[] },
    Term { source: "Parakeet", keep: true, fixed: &[] },
];

/// The terms `english` uses.
pub fn matching(english: &str) -> Vec<&'static Term> {
    let words: Vec<&str> = english
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .collect();
    let has = |term: &Term| {
        let wanted: Vec<&str> = term.source.split(' ').collect();
        words.windows(wanted.len()).any(|window| window == wanted.as_slice())
    };
    TERMS.iter().filter(|term| has(term)).collect()
}
