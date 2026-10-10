use crate::{BotId, Playbook, PlaybookStep, starter_crew, starter_playbooks};

#[test]
fn deliver_names_five_bots_that_exist() {
    let deliver = &starter_playbooks()[0];
    assert_eq!(deliver.steps.len(), 5);
    assert!(deliver.problems(&starter_crew()).is_empty());
    assert!(
        deliver.steps.last().unwrap().asks_first,
        "the Shipper asks first"
    );
}

#[test]
fn design_is_the_second_starter_and_goes_from_research_to_the_check() {
    let playbooks = starter_playbooks();
    assert_eq!(
        playbooks.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["deliver", "design"]
    );
    let design = &playbooks[1];
    assert_eq!(design.name, "Design");
    assert_eq!(
        design
            .steps
            .iter()
            .map(|s| s.bot.as_str())
            .collect::<Vec<_>>(),
        ["quill", "mimi", "bolt", "pip"]
    );
    assert!(design.problems(&starter_crew()).is_empty());
    assert!(design.steps.iter().all(|s| !s.asks_first));
}

#[test]
fn a_step_that_names_a_missing_bot_is_listed_with_its_place() {
    let playbook = Playbook {
        id: "p".parse().unwrap(),
        name: "P".into(),
        steps: vec![PlaybookStep {
            bot: "ghost".parse::<BotId>().unwrap(),
            asks_first: false,
        }],
    };
    let problems = playbook.problems(&starter_crew());
    assert!(
        problems[0].contains("step 1") && problems[0].contains("ghost"),
        "{problems:?}"
    );
}

#[test]
fn a_playbook_with_no_steps_is_refused() {
    let playbook = Playbook {
        id: "p".parse().unwrap(),
        name: "P".into(),
        steps: vec![],
    };
    assert!(
        playbook
            .problems(&starter_crew())
            .iter()
            .any(|p| p.contains("at least one step"))
    );
}
