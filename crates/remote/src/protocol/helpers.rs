use std::io::{self, Read, Write};

use super::types::{Frame, MAX_FRAME};

/// Writes `frame` whole and flushes it.
pub fn write_frame(out: &mut impl Write, frame: &Frame) -> io::Result<()> {
    let bytes = postcard::to_stdvec(frame).map_err(io::Error::other)?;
    let length = frame_length(bytes.len())?;
    out.write_all(&length.to_le_bytes())?;
    out.write_all(&bytes)?;
    out.flush()
}

/// The length prefix for a frame of `bytes` bytes: the writer refuses what the reader would refuse.
pub(super) fn frame_length(bytes: usize) -> io::Result<u32> {
    match u32::try_from(bytes) {
        Ok(length) if bytes <= MAX_FRAME => Ok(length),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("a frame of {bytes} bytes is over the limit"))),
    }
}

/// The next frame, or `None` when the pipe closed between frames. A pipe that closes inside a frame, its
/// length too, is an error.
pub fn read_frame(input: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut length = [0u8; 4];
    loop {
        match input.read(&mut length[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    input.read_exact(&mut length[1..])?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a frame of {length} bytes is over the limit")));
    }
    let mut bytes = vec![0u8; length];
    input.read_exact(&mut bytes)?;
    postcard::from_bytes(&bytes).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
