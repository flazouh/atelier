/// One choice of a question: a short label, and what picking it means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}
/// A question with its choices. `header` is the short word that names it, such as "Crates"; `multi_select` lets the reader
/// pick more than one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
}
