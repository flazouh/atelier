use serde_json::{Map, Value};

use super::{Question, QuestionOption};

/// The questions of an `AskUserQuestion` input, whole or cut: what is there is read, and what is missing is empty. A question
/// with no text yet is not listed.
pub fn questions_of(input: &Value) -> Vec<Question> {
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let list = input.get("questions").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    list.iter()
        .map(|q| Question {
            question: text(q, "question"),
            header: text(q, "header"),
            options: q
                .get("options")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|o| QuestionOption { label: text(o, "label"), description: text(o, "description") })
                .filter(|o| !o.label.is_empty())
                .collect(),
            multi_select: q.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
        })
        .filter(|q| !q.question.is_empty())
        .collect()
}

/// `input` with the reader's answers added, as `claude` takes them back: each question's text mapped to the label picked, or
/// to the labels joined by a comma for a question that takes several.
pub fn answers_input(input: &Value, answers: &[(String, String)]) -> Value {
    let mut input = input.clone();
    let map: Map<String, Value> = answers.iter().map(|(question, answer)| (question.clone(), Value::String(answer.clone()))).collect();
    if let Some(object) = input.as_object_mut() {
        object.insert("answers".into(), Value::Object(map));
    }
    input
}
