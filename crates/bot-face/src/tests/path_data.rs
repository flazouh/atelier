use crate::enums::PathCmd;
use crate::impls::parse_path_data_for_tests as parse;

#[test]
fn absolute_and_relative_steps_become_absolute() {
    let cmds = parse("M10 10H20V30h-5Z").unwrap();
    assert_eq!(
        cmds,
        vec![
            PathCmd::Move(10.0, 10.0),
            PathCmd::Line(20.0, 10.0),
            PathCmd::Line(20.0, 30.0),
            PathCmd::Line(15.0, 30.0),
            PathCmd::Close
        ]
    );
}

#[test]
fn a_quad_keeps_its_control_point() {
    let cmds = parse("M0 0Q5 10 10 0").unwrap();
    assert_eq!(cmds[1], PathCmd::Quad(5.0, 10.0, 10.0, 0.0));
}

#[test]
fn a_negative_number_needs_no_space() {
    let cmds = parse("M-4 -4L4 4").unwrap();
    assert_eq!(
        cmds,
        vec![PathCmd::Move(-4.0, -4.0), PathCmd::Line(4.0, 4.0)]
    );
}

#[test]
fn an_arc_is_refused_by_name() {
    assert!(parse("M0 0A5 5 0 0 1 10 0").unwrap_err().contains("`A`"));
}

#[test]
fn each_relative_pair_builds_on_the_last() {
    let cmds = parse("m1 1 2 0 0 3").unwrap();
    assert_eq!(
        cmds,
        vec![
            PathCmd::Move(1.0, 1.0),
            PathCmd::Line(3.0, 1.0),
            PathCmd::Line(3.0, 4.0)
        ]
    );
}

#[test]
fn a_close_returns_to_the_start_of_the_sub_path() {
    let cmds = parse("M2 2L6 2Zh3").unwrap();
    assert_eq!(cmds.last(), Some(&PathCmd::Line(5.0, 2.0)));
}

#[test]
fn a_cubic_keeps_both_control_points_and_moves_the_pen_to_its_end() {
    let cmds = parse("M26 62C26 43 41 28 60 28L70 28").unwrap();
    assert_eq!(cmds[1], PathCmd::Cubic(26.0, 43.0, 41.0, 28.0, 60.0, 28.0));
    assert_eq!(cmds[2], PathCmd::Line(70.0, 28.0));
}

#[test]
fn a_relative_cubic_adds_to_the_point_where_it_starts() {
    let cmds = parse("M10 10c0 5 5 10 10 10").unwrap();
    assert_eq!(cmds[1], PathCmd::Cubic(10.0, 15.0, 15.0, 20.0, 20.0, 20.0));
}
