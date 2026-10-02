/// The longest recording kept, in seconds. The model reads one clip at a time, and a clip this long is already five minutes of
/// talking; past it the oldest audio is not kept, rather than the buffer growing for ever.
pub const MAX_SECONDS: usize = 300;
