//! Names a file or the whole project writes down, and where: `textDocument/documentSymbol` for
//! "Names in this file" and `workspace/symbol` for "Go to name".
//!
//! Servers answer in two shapes: a tree of `DocumentSymbol`s, or a flat list of `SymbolInformation`s.
//! Both come out as one flat list of [`Symbol`]s in text order, each naming what it sits inside.

use lsp_types::{DocumentSymbol, Range, SymbolInformation, SymbolKind, Uri};

/// One name, where it is written down, and what it sits inside ("impl Server", "mod http"), empty at
/// the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub container: String,
    pub uri: Uri,
    /// The name itself, in the server's position encoding until the worker converts it.
    pub range: Range,
}

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
fn sort(symbols: &mut [Symbol]) {
    symbols.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(b.uri.as_str(), b.range.start.line, b.range.start.character))
    });
}

#[cfg(test)]
mod tests;
