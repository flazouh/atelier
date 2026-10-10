use crate::enums::{Geometry, Paint, Token};
use crate::impls::parse_fragment_for_tests as parse;

#[test]
fn a_rect_reads_its_numbers_and_defaults_to_the_body_colour() {
    let shapes = parse(r#"<rect x="1" y="2" width="3" height="4" rx="1"/>"#).unwrap();
    assert_eq!(
        shapes[0].geometry,
        Geometry::Rect {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            r: 1.0
        }
    );
    assert_eq!(shapes[0].fill, Some(Paint::Token(Token::Body)));
}

#[test]
fn tokens_none_and_a_rotate_are_read() {
    let shapes = parse(r#"<rect x="0" y="0" width="1" height="1" fill="{shade}" transform="rotate(45 60 94)"/><circle cx="1" cy="1" r="1" fill="none" stroke="{body}" stroke-width="4" stroke-linecap="round"/>"#).unwrap();
    assert_eq!(shapes[0].fill, Some(Paint::Token(Token::Shade)));
    assert_eq!(shapes[0].spin, Some([45.0, 60.0, 94.0]));
    assert_eq!(shapes[1].fill, None);
    let stroke = shapes[1].stroke.unwrap();
    assert_eq!(
        (stroke.width, stroke.round_cap, stroke.round_join),
        (4.0, true, false)
    );
}

#[test]
fn an_opacity_and_a_fill_opacity_multiply() {
    let shapes =
        parse(r#"<rect x="0" y="0" width="1" height="1" opacity=".5" fill-opacity=".5"/>"#)
            .unwrap();
    assert_eq!(shapes[0].opacity, 0.25);
}

#[test]
fn an_unknown_shape_is_refused_with_its_name() {
    assert!(parse("<text>x</text>").unwrap_err().contains("`text`"));
}

#[test]
fn a_literal_colour_is_kept() {
    let shapes = parse(r##"<rect x="0" y="0" width="1" height="1" fill="#FFE9B0"/>"##).unwrap();
    assert!(matches!(shapes[0].fill, Some(Paint::Literal(_))));
}

#[test]
fn an_ellipse_reads_its_two_radii() {
    let shapes = parse(r##"<ellipse cx="60" cy="62" rx="37" ry="33" fill="{body}" stroke="#141413" stroke-width="3.2"/>"##).unwrap();
    assert_eq!(
        shapes[0].geometry,
        Geometry::Ellipse {
            cx: 60.0,
            cy: 62.0,
            rx: 37.0,
            ry: 33.0
        }
    );
    assert!(shapes[0].stroke.is_some());
}

#[test]
fn the_ink_token_names_the_outline_colour_the_theme_sets() {
    let shapes = parse(r##"<rect x="0" y="0" width="1" height="1" fill="{body}" stroke="{ink}" stroke-width="3.2"/>"##).unwrap();
    assert_eq!(shapes[0].stroke.map(|s| s.paint), Some(Paint::Token(Token::Ink)));
}
