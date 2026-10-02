use super::types::WORDS;

/// The answer after `n` tokens: three words to a token, a blank line every 14 tokens.
pub(super) fn answer(n: usize) -> String {
    let mut out = String::new();
    for i in 0..n {
        for k in 0..3 {
            out.push_str(WORDS[(i * 3 + k) % WORDS.len()]);
            out.push(' ');
        }
        if i % 14 == 13 {
            out.push_str("\n\n");
        }
    }
    out
}
