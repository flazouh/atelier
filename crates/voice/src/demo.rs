//! SPIKE: a made-up speech model and a made-up voice, so the dictation look can be seen before the real model is wired in.
//! The gallery and the app (behind `ATELIER_DICTATION_SPIKE`) both use it.

/// How long the made-up download, load and ready beat take, in seconds.
pub const DOWNLOAD: f32 = 4.4;
pub const PREPARE: f32 = 1.8;
pub const READY: f32 = 0.7;

/// What the made-up voice "said".
pub const TRANSCRIPT: &str = "Make the tool cards share one header, and keep the read and search rows flat.";

/// The made-up download: quick, a stall at about 60%, quick again. `t` is 0 to 1 through it.
pub fn download_at(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t >= 1. {
        return 1.;
    }
    if t < 0.55 {
        (t / 0.55).powf(0.9) * 0.6
    } else if t < 0.72 {
        0.6 + (t - 0.55) * 0.05
    } else {
        0.6085 + (t - 0.72) / 0.28 * (1. - 0.6085)
    }
}

/// A made-up voice, `seconds` in: words of a few syllables with a breath between them. 0 to 1.
pub fn level(seconds: f32) -> f32 {
    let t = seconds;
    let speaking = ((t * 1.15).sin() + 0.35 * (t * 0.37).sin()) > -0.25;
    let syllables = (t * 5.3).sin().abs() * (0.6 + 0.4 * (t * 1.7 + 1.).sin());
    let grit = 0.06 * (t * 31.).sin().abs();
    if speaking { (0.22 + 0.7 * syllables + grit).min(1.) } else { 0.04 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_download_only_goes_forward_and_ends_full() {
        let mut last = 0.;
        for k in 0..=100 {
            let now = download_at(k as f32 / 100.);
            assert!(now >= last, "{now} < {last}");
            last = now;
        }
        assert_eq!(download_at(1.), 1.);
        assert_eq!(download_at(0.), 0.);
    }

    #[test]
    fn the_download_stalls_in_the_middle() {
        assert!(download_at(0.71) - download_at(0.56) < 0.02);
    }

    #[test]
    fn the_voice_stays_in_range() {
        assert!((0..2000).all(|k| (0. ..=1.).contains(&level(k as f32 * 0.05))));
    }
}
