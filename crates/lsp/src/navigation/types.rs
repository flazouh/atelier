/// Which question the answer is to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    Definition,
    References,
}
