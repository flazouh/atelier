use std::io;

/// `bytes` random bytes from the system, as lowercase hex. A token that could be guessed is worse than no token, so a
/// system with no random source is an error and never a weaker token.
pub(crate) fn random_hex(bytes: usize) -> io::Result<String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|e| io::Error::other(format!("no random bytes: {e}")))?;
    Ok(buffer.iter().map(|b| format!("{b:02x}")).collect())
}
