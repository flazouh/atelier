/// A colour the data names and the caller supplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Body,
    Shade,
    Light,
    Eye,
    /// The outline of every body and part: the theme's ink, so it stays visible on a dark page.
    Ink,
}
