use super::*;

fn uri(path: &str) -> Uri {
    path.parse().expect("a valid uri")
}

fn at(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn range(line: u32, from: u32, to: u32) -> Range {
    Range { start: at(line, from), end: at(line, to) }
}

fn target(path: &str, line: u32, from: u32, to: u32) -> Target {
    let file = path.strip_prefix("file://").map(PathBuf::from);
    Target { uri: uri(path), path: file, range: range(line, from, to), line_text: String::new() }
}

#[test]
fn no_answer_is_no_links() {
    assert!(definition_links(None).is_empty());
}

#[test]
fn a_plain_location_becomes_a_link_that_selects_its_range() {
    let location = Location { uri: uri("file:///a.rs"), range: range(4, 7, 12) };
    let links = definition_links(Some(GotoDefinitionResponse::Scalar(location)));
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_selection_range, range(4, 7, 12));
    assert_eq!(links[0].origin_selection_range, None, "we never invent which word was clicked");
}

#[test]
fn every_location_in_a_list_is_kept_in_order() {
    let locations = vec![
        Location { uri: uri("file:///a.rs"), range: range(1, 0, 1) },
        Location { uri: uri("file:///b.rs"), range: range(9, 0, 1) },
    ];
    let links = definition_links(Some(GotoDefinitionResponse::Array(locations)));
    assert_eq!(links.iter().map(|l| l.target_uri.as_str()).collect::<Vec<_>>(), ["file:///a.rs", "file:///b.rs"]);
}

#[test]
fn a_link_answer_keeps_its_own_name_range() {
    let link = LocationLink {
        origin_selection_range: Some(range(6, 4, 9)),
        target_uri: uri("file:///a.rs"),
        target_range: range(0, 0, 30),
        target_selection_range: range(0, 7, 12),
    };
    assert_eq!(definition_links(Some(GotoDefinitionResponse::Link(vec![link.clone()]))), vec![link]);
}

#[test]
fn an_empty_answer_has_nowhere_to_go() {
    assert!(lands_on_itself(&[], Path::new("/a.rs"), at(0, 3)));
}

#[test]
fn the_caret_on_the_declaration_lands_on_itself() {
    let here = Path::new("/a.rs");
    let declaration = [target("file:///a.rs", 0, 7, 12)];
    assert!(lands_on_itself(&declaration, here, at(0, 9)), "inside the name");
    assert!(lands_on_itself(&declaration, here, at(0, 12)), "just after the name");
}

#[test]
fn a_definition_elsewhere_is_somewhere_to_go() {
    let here = Path::new("/a.rs");
    assert!(!lands_on_itself(&[target("file:///a.rs", 0, 7, 12)], here, at(6, 9)), "another line");
    assert!(!lands_on_itself(&[target("file:///b.rs", 0, 7, 12)], here, at(0, 9)), "another file");
    let mixed = [target("file:///a.rs", 0, 7, 12), target("file:///b.rs", 3, 0, 4)];
    assert!(!lands_on_itself(&mixed, here, at(0, 9)), "one real target is enough");
}

#[test]
fn targets_read_by_file_then_position() {
    let mut targets =
        vec![target("file:///b.rs", 1, 0, 1), target("file:///a.rs", 9, 0, 1), target("file:///a.rs", 2, 4, 5)];
    sort_targets(&mut targets);
    let order: Vec<_> = targets.iter().map(|t| (t.uri.as_str().to_string(), t.range.start.line)).collect();
    assert_eq!(order, [("file:///a.rs".into(), 2), ("file:///a.rs".into(), 9), ("file:///b.rs".into(), 1)]);
}

#[test]
fn the_same_file_escaped_differently_is_still_itself() {
    // typescript-language-server writes `@` as `%40`; the path, not the URI string, decides.
    let target = Target {
        uri: uri("file:///work/%40scope/a.ts"),
        path: Some(PathBuf::from("/work/@scope/a.ts")),
        range: range(0, 16, 21),
        line_text: String::new(),
    };
    assert!(lands_on_itself(&[target], Path::new("/work/@scope/a.ts"), at(0, 18)));
}
