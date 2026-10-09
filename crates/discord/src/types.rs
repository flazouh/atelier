/// Why a runner could not give an answer at all. A command that ran and failed is an [`Output`](crate::Output) with a
/// code, not this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The program is not there.
    NotInstalled(String),
    /// It started and the system failed it.
    Io(String),
}
