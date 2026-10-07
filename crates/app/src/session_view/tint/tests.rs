use super::{POOL, next};

#[test]
fn a_new_subagent_takes_the_first_colour_nobody_holds() {
    assert_eq!(next(&[]), 0);
    assert_eq!(next(&[0, 1]), 2);
    assert_eq!(next(&[1, 0, 3]), 2, "a colour that came back is used again before a new one");
}

#[test]
fn a_colour_repeats_only_when_the_pool_is_empty() {
    let all: Vec<usize> = (0..POOL.len()).collect();
    assert_eq!(next(&all), 0, "every colour held once: the first again");
    let mut busy = all.clone();
    busy.push(0);
    assert_eq!(next(&busy), 1, "the colour held least");
}
