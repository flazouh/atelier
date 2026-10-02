use super::*;

#[test]
fn a_frame_crosses_whole_and_the_pipe_can_close_between_frames() {
    let frames = [
        Frame::Request { id: 1, call: Call::Hello { version: VERSION, root: "/home/a/p".into() } },
        Frame::Request { id: 2, call: Call::Write { path: "a.bin".into(), bytes: vec![0, 1, 2, 255] } },
        Frame::Response { id: 2, result: Err(Failure { kind: FailureKind::NotFound, message: "gone".into() }) },
        Frame::Event(Event::Exited { pid: 7, code: Some(3) }),
    ];
    let mut pipe = Vec::new();
    for frame in &frames {
        write_frame(&mut pipe, frame).unwrap();
    }
    let mut input = pipe.as_slice();
    for frame in &frames {
        assert_eq!(read_frame(&mut input).unwrap().as_ref(), Some(frame));
    }
    assert_eq!(read_frame(&mut input).unwrap(), None, "closed between frames");
}

#[test]
fn a_cut_frame_or_a_huge_length_is_an_error_not_a_hang() {
    let mut pipe = Vec::new();
    write_frame(&mut pipe, &Frame::Request { id: 1, call: Call::List }).unwrap();
    let cut = &pipe[..pipe.len() - 1];
    assert!(read_frame(&mut &cut[..]).is_err());
    let huge = (MAX_FRAME as u32 + 1).to_le_bytes();
    assert_eq!(read_frame(&mut &huge[..]).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn a_failure_keeps_its_kind_across() {
    let error = io::Error::new(io::ErrorKind::NotFound, "no such file");
    let back: io::Error = Failure::from(&error).into();
    assert_eq!(back.kind(), io::ErrorKind::NotFound);
    assert_eq!(back.to_string(), "no such file");
}

/// A pipe that closes inside the four bytes of a length is a cut frame, not a pipe that closed between frames.
#[test]
fn a_pipe_that_closes_inside_a_length_is_an_error() {
    for cut in 1..4 {
        let bytes = [7u8; 4];
        let error = read_frame(&mut &bytes[..cut]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof, "{cut} bytes");
    }
    assert_eq!(read_frame(&mut &[][..]).unwrap(), None);
}

/// The writer refuses what the reader refuses, so a big frame fails where it is written.
#[test]
fn a_frame_over_the_limit_is_refused_by_the_writer() {
    assert!(frame_length(MAX_FRAME).is_ok());
    assert_eq!(frame_length(MAX_FRAME + 1).unwrap_err().kind(), io::ErrorKind::InvalidInput);
}
