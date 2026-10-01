use std::io::{self, Read, Write};

use super::types::{Frame, MAX_FRAME};

/// Writes `frame` whole and flushes it.
pub fn write_frame(out: &mut impl Write, frame: &Frame) -> io::Result<()> {
    let bytes = postcard::to_stdvec(frame).map_err(io::Error::other)?;
    let length = u32::try_from(bytes.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a frame over 4GB"))?;
    out.write_all(&length.to_le_bytes())?;
    out.write_all(&bytes)?;
    out.flush()
}

/// The next frame, or `None` when the pipe closed between frames.
pub fn read_frame(input: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut length = [0u8; 4];
    match input.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a frame of {length} bytes is over the limit")));
    }
    let mut bytes = vec![0u8; length];
    input.read_exact(&mut bytes)?;
    postcard::from_bytes(&bytes).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
