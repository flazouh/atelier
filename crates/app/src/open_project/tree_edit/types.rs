/// What the name typed in the tree is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeEditKind {
    /// A new file in the folder `parent` (`""` is the project's folder).
    NewFile { parent: String },
    NewFolder { parent: String },
    /// A new name for the file or folder at `path`.
    Rename { path: String },
}
