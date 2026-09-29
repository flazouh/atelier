//! How many files differ from the last commit, from `git status --porcelain -z`: the status line's dirty
//! count. Pure.

/// The number of entries in `porcelain` (`git status --porcelain -z` output). A rename or a copy carries
/// its old path as a second field, which is not a file of its own.
pub fn count(porcelain: &str) -> usize {
    let mut fields = porcelain.split('\0').filter(|f| !f.is_empty());
    let mut n = 0;
    while let Some(entry) = fields.next() {
        n += 1;
        if entry.starts_with(['R', 'C']) {
            fields.next();
        }
    }
    n
}

#[cfg(test)]
mod tests;
