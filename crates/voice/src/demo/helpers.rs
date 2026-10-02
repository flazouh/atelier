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
