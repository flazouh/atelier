use super::*;

#[test]
fn stereo_is_mixed_to_one_channel_by_averaging() {
    assert_eq!(mono(&[1., 0., 0.5, 0.5, -1., 1.], 2), [0.5, 0.5, 0.]);
    assert_eq!(mono(&[0.1, 0.2], 1), [0.1, 0.2]);
}

#[test]
fn audio_at_16k_is_left_alone() {
    let audio: Vec<f32> = (0..100).map(|i| i as f32 / 100.).collect();
    assert_eq!(to_16k(&audio, 16_000), audio);
}

#[test]
fn audio_at_48k_comes_out_a_third_as_long_and_keeps_its_level() {
    let audio = vec![0.5f32; 4800];
    let out = to_16k(&audio, 48_000);
    assert_eq!(out.len(), 1600);
    assert!(out.iter().all(|s| (s - 0.5).abs() < 1e-5));
}

#[test]
fn audio_at_44k1_has_the_right_length_and_a_tone_keeps_its_loudness() {
    let rate = 44_100u32;
    let tone: Vec<f32> = (0..rate as usize).map(|i| (i as f32 / rate as f32 * 440. * std::f32::consts::TAU).sin() * 0.8).collect();
    let out = to_16k(&tone, rate);
    assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
    let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
    assert!(peak > 0.75 && peak <= 0.81, "{peak}");
}

#[test]
fn a_tone_above_the_new_nyquist_is_cut_down_not_folded_in() {
    // 12 kHz at 48 kHz would alias to 4 kHz at 16 kHz if it were simply picked from.
    let rate = 48_000u32;
    let tone: Vec<f32> = (0..rate as usize).map(|i| (i as f32 / rate as f32 * 12_000. * std::f32::consts::TAU).sin()).collect();
    let peak = to_16k(&tone, rate).iter().fold(0f32, |m, s| m.max(s.abs()));
    assert!(peak < 0.75, "{peak}");
}

#[test]
fn silence_has_no_level_and_loud_speech_is_near_full() {
    assert_eq!(level_from_rms(0.), 0.);
    assert_eq!(level_from_rms(0.001), 0.); // -60 dB
    assert!(level_from_rms(0.3) > 0.9); // about -10.5 dB
    assert!(level_from_rms(0.02) > 0.3 && level_from_rms(0.02) < 0.8); // -34 dB, ordinary speech
}

#[test]
fn the_level_grows_with_the_loudness() {
    let at = |rms: f32| level_from_rms(rms);
    assert!(at(0.01) < at(0.03) && at(0.03) < at(0.1) && at(0.1) <= at(0.5));
}

#[test]
fn the_rms_covers_what_came_since_it_was_last_read() {
    let mut heard = Heard::default();
    heard.push(&[0.5, -0.5, 0.5, -0.5], 100);
    assert!((heard.take_rms() - 0.5).abs() < 1e-6);
    assert_eq!(heard.take_rms(), 0., "nothing came since");
    heard.push(&[0.1; 10], 100);
    assert!((heard.take_rms() - 0.1).abs() < 1e-6);
}

#[test]
fn a_long_recording_keeps_only_the_newest_audio() {
    let mut heard = Heard::default();
    heard.push(&[1., 2., 3., 4.], 6);
    heard.push(&[5., 6., 7., 8.], 6);
    assert_eq!(heard.samples, [3., 4., 5., 6., 7., 8.]);
}
