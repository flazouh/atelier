use super::structs::Shown;

/// What a file shows as.
#[derive(Clone, Debug)]
pub enum Content {
    Text(Box<Shown>),
    /// Not text: listed, no rows.
    Binary,
    TooLarge(u64),
}
