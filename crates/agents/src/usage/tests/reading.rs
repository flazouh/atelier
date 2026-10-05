use super::super::Reading;

#[test]
fn a_percent_outside_the_range_is_held_in_it_and_not_a_number_is_none_used() {
    let reading = Reading::default().window("a", 250., None).window("b", -5., None).window("c", f64::NAN, None);
    assert_eq!(reading.windows.iter().map(|w| w.used).collect::<Vec<_>>(), [1., 0., 0.]);
}
