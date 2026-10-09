/// How many characters of a body the screen draws before it offers "Show all". A body of a mailing list may be a megabyte, and
/// laying it out would stop the window.
pub const BODY_LIMIT: usize = 4_000;

/// How many addresses of one message the screen lists as links.
pub const LINKS_MOST: usize = 5;
