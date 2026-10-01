use lsp_types::{DocumentSymbol, SymbolInformation, Uri};

use super::structs::Symbol;

/// A tree of a file's symbols, flattened in text order, each child naming its parent.
pub fn flatten(uri: &Uri, tree: Vec<DocumentSymbol>) -> Vec<Symbol> {
    fn walk(uri: &Uri, container: &str, symbols: Vec<DocumentSymbol>, out: &mut Vec<Symbol>) {
        for symbol in symbols {
            out.push(Symbol {
                name: symbol.name.clone(),
                kind: symbol.kind,
                container: container.to_string(),
                uri: uri.clone(),
                range: symbol.selection_range,
            });
            walk(uri, &symbol.name, symbol.children.unwrap_or_default(), out);
        }
    }
    let mut out = Vec::new();
    walk(uri, "", tree, &mut out);
    sort(&mut out);
    out
}

/// A flat answer as symbols.
#[allow(deprecated)] // `SymbolInformation::deprecated` is a field the type still has.
pub fn from_information(list: Vec<SymbolInformation>) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = list
        .into_iter()
        .map(|s| Symbol {
            name: s.name,
            kind: s.kind,
            container: s.container_name.unwrap_or_default(),
            uri: s.location.uri,
            range: s.location.range,
        })
        .collect();
    sort(&mut out);
    out
}

/// By file, then by where in it.
pub(super) fn sort(symbols: &mut [Symbol]) {
    symbols.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(b.uri.as_str(), b.range.start.line, b.range.start.character))
    });
}
