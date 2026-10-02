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
