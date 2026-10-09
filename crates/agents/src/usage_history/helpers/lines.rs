use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use crate::usage_history::consts::READ_BUFFER_BYTES;

/// Calls `f` with each non-empty trimmed line. Bytes that are not UTF-8 become U+FFFD, so one bad byte costs
/// one line at most.
pub(crate) fn for_each_line(path: &Path, mut f: impl FnMut(&str)) -> io::Result<()> {
    let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, File::open(path)?);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim();
        if !line.is_empty() {
            f(line);
        }
    }
}
