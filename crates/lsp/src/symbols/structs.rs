use lsp_types::{Range, SymbolKind, Uri};

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
