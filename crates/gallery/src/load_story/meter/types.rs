/// How many frames a measuring run lasts.
pub const FRAMES: usize = 300;

/// The frame interval a frame must fit: 120Hz. `GALLERY_FRAME_MS` sets another, such as 16.67 for 60Hz.
pub const FRAME_MS: f64 = 1000. / 120.;
