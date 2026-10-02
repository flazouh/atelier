use super::*;

#[test]
fn a_glide_covers_most_of_the_gap_in_beuis_320ms_and_lands_exactly() {
    let mut y = 0.;
    let mut frames = 0;
    while y != 400. {
        y = step(y, 400., 1. / 60.);
        frames += 1;
        if frames == 20 {
            assert!(y > 390., "after 320ms it is nearly there: {y}");
        }
    }
    assert!(frames < 40, "and it lands, in {frames} frames");
}

#[test]
fn each_frame_takes_part_of_the_gap_so_a_moving_end_is_chased_smoothly() {
    let first = step(0., 400., 1. / 60.);
    assert!(first > 0. && first < 100., "{first}");
    assert_eq!(step(399.7, 400., 1. / 60.), 400., "under half a pixel it lands");
    assert!(step(400., 0., 1. / 60.) < 400., "it goes up as well as down");
}

#[test]
fn a_message_goes_to_the_middle_of_the_view_inside_the_list() {
    assert_eq!(centred(1000., 100., 600., 5000.), 750.);
    assert_eq!(centred(100., 100., 600., 5000.), 0., "not above the top");
    assert_eq!(centred(4900., 100., 600., 4500.), 4500., "not past the end");
    assert_eq!(centred(1000., 900., 600., 5000.), 1150., "a tall message shows its middle, as beui's does");
}
