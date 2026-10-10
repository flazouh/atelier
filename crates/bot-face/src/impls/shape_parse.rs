use gpui_kit::Rgba;

use super::path_data::parse_path_data;
use crate::enums::{Geometry, Paint, Token};
use crate::structs::{Shape, StrokeSpec};

/// Reads the shapes of one SVG fragment: `rect`, `circle`, `polygon` and `path`, with fill, stroke, opacity and
/// a rotate transform. A token such as `{body}` names a colour the caller gives.
pub(crate) fn parse_fragment(svg: &str) -> Result<Vec<Shape>, String> {
    let wrapped = format!("<g>{svg}</g>");
    let doc =
        roxmltree::Document::parse(&wrapped).map_err(|e| format!("the svg does not parse: {e}"))?;
    doc.root_element()
        .children()
        .filter(|n| n.is_element())
        .map(|n| shape_of(&n))
        .collect()
}

fn shape_of(node: &roxmltree::Node) -> Result<Shape, String> {
    let num = |name: &str, default: f32| -> Result<f32, String> {
        match node.attribute(name) {
            Some(v) => v
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("`{name}` is not a number: `{v}`")),
            None => Ok(default),
        }
    };
    let geometry = match node.tag_name().name() {
        "rect" => Geometry::Rect {
            x: num("x", 0.0)?,
            y: num("y", 0.0)?,
            w: num("width", 0.0)?,
            h: num("height", 0.0)?,
            r: num("rx", 0.0)?,
        },
        "circle" => Geometry::Circle {
            cx: num("cx", 0.0)?,
            cy: num("cy", 0.0)?,
            r: num("r", 0.0)?,
        },
        "polygon" => Geometry::Polygon(parse_points(node.attribute("points").unwrap_or(""))?),
        "path" => Geometry::Path(parse_path_data(node.attribute("d").unwrap_or(""))?),
        other => return Err(format!("the shape `{other}` is not supported")),
    };
    let fill = match node.attribute("fill") {
        None => Some(Paint::Token(Token::Body)),
        Some(v) => parse_paint(v)?,
    };
    let fill_opacity = num("fill-opacity", 1.0)?;
    let stroke = match node.attribute("stroke") {
        Some(v) => parse_paint(v)?.map(|paint| StrokeSpec {
            paint,
            width: num("stroke-width", 1.0).unwrap_or(1.0),
            round_cap: node.attribute("stroke-linecap") == Some("round"),
            round_join: node.attribute("stroke-linejoin") == Some("round"),
        }),
        None => None,
    };
    Ok(Shape {
        geometry,
        fill,
        stroke,
        opacity: num("opacity", 1.0)? * fill_opacity,
        spin: parse_spin(node.attribute("transform"))?,
    })
}

fn parse_points(text: &str) -> Result<Vec<(f32, f32)>, String> {
    let numbers = text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<f32>()
                .map_err(|_| format!("bad point number `{s}`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if numbers.len() % 2 != 0 {
        return Err("a polygon has an odd number of coordinates".into());
    }
    Ok(numbers.chunks(2).map(|p| (p[0], p[1])).collect())
}

/// `none` is no paint, `{name}` is a token, and `#rrggbb` is a literal colour.
fn parse_paint(value: &str) -> Result<Option<Paint>, String> {
    match value.trim() {
        "none" => Ok(None),
        "{body}" => Ok(Some(Paint::Token(Token::Body))),
        "{shade}" => Ok(Some(Paint::Token(Token::Shade))),
        "{light}" => Ok(Some(Paint::Token(Token::Light))),
        "{eye}" => Ok(Some(Paint::Token(Token::Eye))),
        other => Rgba::try_from(other)
            .map(|c| Some(Paint::Literal(c)))
            .map_err(|e| format!("the colour `{other}` is not known: {e}")),
    }
}

fn parse_spin(value: Option<&str>) -> Result<Option<[f32; 3]>, String> {
    let Some(text) = value else { return Ok(None) };
    let inner = text
        .trim()
        .strip_prefix("rotate(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| format!("only rotate(a cx cy) is supported, got `{text}`"))?;
    let v = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<f32>()
                .map_err(|_| format!("bad rotate number `{s}`"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    match v.as_slice() {
        [a, x, y] => Ok(Some([*a, *x, *y])),
        [a] => Ok(Some([*a, 0.0, 0.0])),
        _ => Err(format!("rotate needs 1 or 3 numbers, got `{text}`")),
    }
}
