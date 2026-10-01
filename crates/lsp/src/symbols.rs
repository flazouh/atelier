//! Names a file or the whole project writes down, and where: `textDocument/documentSymbol` for
//! "Names in this file" and `workspace/symbol` for "Go to name".
//!
//! Servers answer in two shapes: a tree of `DocumentSymbol`s, or a flat list of `SymbolInformation`s.
//! Both come out as one flat list of [`Symbol`]s in text order, each naming what it sits inside.

mod helpers;
mod structs;

pub use helpers::{flatten, from_information};
pub use structs::Symbol;

#[cfg(test)]
use lsp_types::{DocumentSymbol, Range, SymbolInformation, SymbolKind, Uri};

#[cfg(test)]
mod tests;
