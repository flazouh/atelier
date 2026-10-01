/// A row pair is worth showing word by word when at least this share of its bytes is the same; below
/// that the rows are two different lines and the whole of each is the change.
pub(super) const MIN_SIMILARITY: f64 = 0.4;
