use crate::enums::PathCmd;
use crate::impls::parse_path_data_for_tests as parse;

#[test]
fn absolute_and_relative_steps_become_absolute() {
    let cmds = parse("M10 10H20V30h-5Z").unwrap();
    assert_eq!(cmds, vec![PathCmd::Move(10.0, 10.0), PathCmd::Line(20.0, 10.0), PathCmd::Line(20.0, 30.0), PathCmd::Line(15.0, 30.0), PathCmd::Close]);
}

#[test]
fn a_quad_keeps_its_control_point() {
    let cmds = parse("M0 0Q5 10 10 0").unwrap();
    assert_eq!(cmds[1], PathCmd::Quad(5.0, 10.0, 10.0, 0.0));
}

#[test]
fn a_negative_number_needs_no_space() {
    let cmds = parse("M-4 -4L4 4").unwrap();
    assert_eq!(cmds, vec![PathCmd::Move(-4.0, -4.0), PathCmd::Line(4.0, 4.0)]);
}

#[test]
fn an_arc_is_refused_by_name() {
    assert!(parse("M0 0A5 5 0 0 1 10 0").unwrap_err().contains("`A`"));
}
