/// One line of a changelog: what it is about, and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLine {
    /// New, improved or fixed: the heading the note stood under.
    pub kind: atelier_ui::ReleaseKind,
    pub lead: String,
    pub text: String,
}
