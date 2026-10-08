use super::structs::Price;

/// Timestamps are kept in buckets of this many seconds (15 minutes: every real UTC offset is a multiple).
pub(super) const BUCKET_SECS: i64 = 900;
pub(super) const SECS_PER_DAY: i64 = 86_400;
/// A file is opened when it changed at or after the start day minus this slack.
pub(super) const MTIME_SLACK_SECS: i64 = SECS_PER_DAY;
pub(super) const MAX_THREADS: usize = 8;
pub(super) const READ_BUFFER_BYTES: usize = 256 * 1024;
pub(super) const TITLE_MAX_CHARS: usize = 60;
pub(super) const UNTITLED: &str = "Untitled";
pub(super) const UNKNOWN_MODEL: &str = "unknown";

pub(super) const PER_MILLION: f64 = 1_000_000.0;

/// ESTIMATED list prices in USD per million tokens, by model family: the first row whose name the model name
/// contains (lower case) wins. Cache read and cache write are a ratio of the input price. Real bills can differ
/// (plan discounts, long-context tiers, 1 hour cache writes), so show these as estimates.
pub(super) const PRICES: &[(&str, Price)] = &[
    ("gpt-5", Price { input: 1.25, output: 10.0, cache_read_ratio: 0.1, cache_write_ratio: 1.0 }),
    ("opus", Price { input: 5.0, output: 25.0, cache_read_ratio: 0.1, cache_write_ratio: 1.25 }),
    ("sonnet", Price { input: 3.0, output: 15.0, cache_read_ratio: 0.1, cache_write_ratio: 1.25 }),
    ("haiku", Price { input: 1.0, output: 5.0, cache_read_ratio: 0.1, cache_write_ratio: 1.25 }),
];
