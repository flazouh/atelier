/// One line of a changelog: what it is about, and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLine {
    pub lead: String,
    pub text: String,
}
