//! The agent's questions as cards: while the question streams in, while it waits for the reader, and once it is answered. The
//! card is the design system's [`QuestionCard`]; this module reads the call's input into it.
use atelier_agents::session::{Answer, Call, ChoiceKind, ToolStatus, questions_of};
use atelier_ui::{QuestionStatus, QuestionView};
use gpui_kit::SharedString;

/// The questions in a call's input, as far as it has come.
pub(super) fn views(input: &serde_json::Value) -> Vec<QuestionView> {
    questions_of(input)
        .into_iter()
        .map(|q| QuestionView {
            header: q.header.into(),
            question: q.question.into(),
            options: q.options.into_iter().map(|o| (o.label.into(), o.description.into())).collect(),
            multiple: q.multi_select,
        })
        .collect()
}

/// How the card of a call's own row stands: streaming until the agent has asked and the request has come, then answered, with
/// the reader's answers when they were given here. `approval` is the call's request, if one came.
pub(super) fn call_state(call: &Call, approval: Option<&Answer>, said: Option<&[(String, String)]>) -> (QuestionStatus, Vec<(SharedString, SharedString)>) {
    let questions = views(&call.call.input);
    match approval {
        None if matches!(call.call.status, ToolStatus::Pending | ToolStatus::Running) => (QuestionStatus::Streaming, Vec::new()),
        Some(Answer::Withdrawn | Answer::Answered(ChoiceKind::Deny)) => {
            (QuestionStatus::Answered, questions.into_iter().map(|q| (q.question, SharedString::from("Not answered"))).collect())
        }
        _ => {
            let answers = questions
                .into_iter()
                .map(|q| {
                    let given = said.and_then(|said| said.iter().find(|(question, _)| q.question == question.as_str())).map(|(_, a)| a.clone());
                    (q.question, given.map(SharedString::from).unwrap_or_default())
                })
                .collect();
            (QuestionStatus::Answered, answers)
        }
    }
}
