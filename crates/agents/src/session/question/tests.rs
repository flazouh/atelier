use serde_json::json;

use super::{Question, QuestionOption, answers_input, questions_of};

fn option(label: &str, description: &str) -> QuestionOption {
    QuestionOption { label: label.into(), description: description.into() }
}

#[test]
fn the_questions_of_an_input_are_read_with_their_options() {
    let input = json!({"questions": [
        {"question": "Which toy?", "header": "Toy", "options": [{"label": "Chain", "description": "Blocks"}, {"label": "Cipher", "description": "XOR"}], "multiSelect": false},
        {"question": "Which crates?", "header": "Crates", "options": [{"label": "Few"}], "multiSelect": true},
    ]});
    assert_eq!(
        questions_of(&input),
        vec![
            Question { question: "Which toy?".into(), header: "Toy".into(), options: vec![option("Chain", "Blocks"), option("Cipher", "XOR")], multi_select: false },
            Question { question: "Which crates?".into(), header: "Crates".into(), options: vec![option("Few", "")], multi_select: true },
        ]
    );
}

#[test]
fn a_cut_input_gives_what_has_come() {
    let cut = crate::partial_json::value(r#"{"questions":[{"question":"Which toy?","header":"To"#).unwrap();
    assert_eq!(questions_of(&cut), vec![Question { question: "Which toy?".into(), header: "To".into(), options: vec![], multi_select: false }]);
    assert!(questions_of(&json!({})).is_empty());
    assert!(questions_of(&crate::partial_json::value(r#"{"questions":[{"que"#).unwrap()).is_empty(), "no question text yet");
}

#[test]
fn the_answers_are_added_to_the_input_by_question_text() {
    let input = json!({"questions": [{"question": "Which toy?"}]});
    let with = answers_input(&input, &[("Which toy?".into(), "Chain".into())]);
    assert_eq!(with["answers"], json!({"Which toy?": "Chain"}));
    assert_eq!(with["questions"], input["questions"], "the questions stay");
}
