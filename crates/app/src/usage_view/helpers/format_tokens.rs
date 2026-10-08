/// A count of tokens, short: `812`, `12.3 K`, `5.84 M`, `1.20 B`.
pub fn format_tokens(tokens: u64) -> String {
    let n = tokens as f64;
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1} K", n / 1e3),
        1_000_000..=999_999_999 => format!("{:.2} M", n / 1e6),
        _ => format!("{:.2} B", n / 1e9),
    }
}
