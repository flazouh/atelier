use std::str::FromStr;

use lsp_types::{Location, Position};

use super::*;

fn range(line: u32) -> Range {
    Range::new(Position::new(line, 4), Position::new(line, 9))
}

#[allow(deprecated)]
fn symbol(name: &str, kind: SymbolKind, line: u32, children: Vec<DocumentSymbol>) -> DocumentSymbol {
    DocumentSymbol {
        name: name.into(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range: Range::new(Position::new(line, 0), Position::new(line + 3, 1)),
        selection_range: range(line),
        children: Some(children),
    }
}

#[test]
fn a_tree_flattens_in_text_order_with_each_parent_named() {
    let uri = Uri::from_str("file:///p/src/lib.rs").unwrap();
    let tree = vec![
        symbol("Server", SymbolKind::STRUCT, 4, vec![]),
        symbol("helpers", SymbolKind::MODULE, 0, vec![symbol("width", SymbolKind::FUNCTION, 1, vec![])]),
    ];
    let names: Vec<(String, String, u32)> =
        flatten(&uri, tree).into_iter().map(|s| (s.name, s.container, s.range.start.line)).collect();
    assert_eq!(
        names,
        [("helpers".into(), "".into(), 0), ("width".into(), "helpers".into(), 1), ("Server".into(), "".into(), 4)]
    );
}

#[test]
#[allow(deprecated)]
fn a_flat_list_keeps_its_containers_and_sorts_by_file_then_line() {
    let at = |path: &str, line| Location { uri: Uri::from_str(path).unwrap(), range: range(line) };
    let info = |name: &str, location, container: Option<&str>| SymbolInformation {
        name: name.into(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        deprecated: None,
        location,
        container_name: container.map(Into::into),
    };
    let list = vec![
        info("b", at("file:///p/src/b.rs", 2), None),
        info("a2", at("file:///p/src/a.rs", 9), Some("impl A")),
        info("a1", at("file:///p/src/a.rs", 1), None),
    ];
    let names: Vec<(String, String)> = from_information(list).into_iter().map(|s| (s.name, s.container)).collect();
    assert_eq!(names, [("a1".into(), "".into()), ("a2".into(), "impl A".into()), ("b".into(), "".into())]);
}
