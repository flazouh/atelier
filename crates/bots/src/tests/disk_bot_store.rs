use crate::{
    Access, BotId, BotStore, BotsError, DiskBotStore, MemoryScope, Playbook, PlaybookStep,
    ToolGrant, seed_starters, starter_crew,
};

fn store() -> (tempfile::TempDir, DiskBotStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = DiskBotStore::new(dir.path().join("bots-data"));
    (dir, store)
}

fn id(text: &str) -> BotId {
    text.parse().unwrap()
}

#[test]
fn seeding_keeps_ten_bots_and_two_playbooks_and_a_second_seeding_adds_nothing() {
    let (_dir, store) = store();
    assert_eq!(seed_starters(&store).unwrap(), 12);
    assert_eq!(store.bots().unwrap().len(), 10);
    assert_eq!(
        store
            .playbooks()
            .unwrap()
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["deliver", "design"]
    );
    assert_eq!(seed_starters(&store).unwrap(), 0);
}

#[test]
fn a_bot_comes_back_as_it_was_kept() {
    let (_dir, store) = store();
    let bolt = starter_crew()
        .into_iter()
        .find(|b| b.id == id("bolt"))
        .unwrap();
    store.save_bot(bolt.clone()).unwrap();
    assert_eq!(store.bot(&id("bolt")).unwrap(), bolt);
}

#[test]
fn an_edit_makes_the_next_version_and_the_same_bot_changes_nothing() {
    let (_dir, store) = store();
    let mut bolt = starter_crew()
        .into_iter()
        .find(|b| b.id == id("bolt"))
        .unwrap();
    store.save_bot(bolt.clone()).unwrap();
    assert_eq!(
        store.save_bot(bolt.clone()).unwrap().version,
        1,
        "the same bot keeps its version"
    );
    bolt.skills.push("eng-qa".into());
    let saved = store.save_bot(bolt.clone()).unwrap();
    assert_eq!(saved.version, 2);
    bolt.version = 99;
    assert_eq!(
        store.save_bot(bolt).unwrap().version,
        2,
        "a caller cannot choose the version of an edit that adds nothing"
    );
}

#[test]
fn a_bot_with_a_problem_is_refused_and_nothing_is_written() {
    let (_dir, store) = store();
    let mut bolt = starter_crew().remove(1);
    bolt.name.clear();
    assert!(matches!(store.save_bot(bolt), Err(BotsError::Invalid(_))));
    assert!(store.bots().unwrap().is_empty());
}

#[test]
fn a_missing_bot_is_not_found_and_a_removed_one_is_gone() {
    let (_dir, store) = store();
    assert!(matches!(
        store.bot(&id("ghost")),
        Err(BotsError::NotFound(_))
    ));
    store.save_bot(starter_crew().remove(1)).unwrap();
    store.remove_bot(&id("bolt")).unwrap();
    assert!(store.bots().unwrap().is_empty());
    assert!(matches!(
        store.remove_bot(&id("bolt")),
        Err(BotsError::NotFound(_))
    ));
}

#[test]
fn a_playbook_can_only_name_bots_that_are_kept() {
    let (_dir, store) = store();
    let playbook = Playbook {
        id: id("solo"),
        name: "Solo".into(),
        steps: vec![PlaybookStep {
            bot: id("bolt"),
            asks_first: false,
        }],
    };
    assert!(matches!(
        store.save_playbook(playbook.clone()),
        Err(BotsError::Invalid(_))
    ));
    store.save_bot(starter_crew().remove(1)).unwrap();
    store.save_playbook(playbook).unwrap();
    assert_eq!(store.playbooks().unwrap().len(), 1);
}

#[test]
fn a_seeded_bot_that_you_edited_is_not_touched_by_a_second_seeding() {
    let (_dir, store) = store();
    seed_starters(&store).unwrap();
    let mut dot = store.bot(&id("dot")).unwrap();
    dot.tools.push(ToolGrant {
        connector: "sentry".into(),
        access: Access::Read,
    });
    store.save_bot(dot).unwrap();
    seed_starters(&store).unwrap();
    assert_eq!(store.bot(&id("dot")).unwrap().tools.len(), 1);
    assert_eq!(store.bot(&id("dot")).unwrap().version, 2);
}

#[test]
fn notes_stay_in_their_layer() {
    let (_dir, store) = store();
    let bot = MemoryScope::Bot(id("dot"));
    let workspace = MemoryScope::Workspace("fluentai".into());
    let project = MemoryScope::Project("/home/a/code/atelier".into());
    store
        .add_note(&bot, "Read the failing test first.", 10)
        .unwrap();
    store.add_note(&workspace, "We squash merge.", 20).unwrap();
    store
        .add_note(&project, "The gateway port is random.", 30)
        .unwrap();
    assert_eq!(
        store
            .notes(&bot)
            .unwrap()
            .iter()
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>(),
        ["Read the failing test first."]
    );
    assert_eq!(store.notes(&workspace).unwrap()[0].text, "We squash merge.");
    assert_eq!(store.notes(&project).unwrap()[0].created_ms, 30);
    assert!(
        store
            .notes(&MemoryScope::Bot(id("bolt")))
            .unwrap()
            .is_empty(),
        "another bot has its own memory"
    );
}

#[test]
fn a_note_number_is_never_used_again_after_a_removal() {
    let (_dir, store) = store();
    let scope = MemoryScope::Bot(id("dot"));
    let first = store.add_note(&scope, "one", 1).unwrap();
    let second = store.add_note(&scope, "two", 2).unwrap();
    store.remove_note(&scope, second.id).unwrap();
    let third = store.add_note(&scope, "three", 3).unwrap();
    assert_eq!((first.id, second.id, third.id), (0, 1, 2));
    assert_eq!(store.notes(&scope).unwrap().len(), 2);
    assert!(matches!(
        store.remove_note(&scope, 77),
        Err(BotsError::NotFound(_))
    ));
}

#[test]
fn an_empty_or_huge_note_is_refused() {
    let (_dir, store) = store();
    let scope = MemoryScope::Bot(id("dot"));
    assert!(store.add_note(&scope, "   ", 1).is_err());
    assert!(store.add_note(&scope, &"x".repeat(2001), 1).is_err());
}

#[test]
fn a_project_path_with_slashes_cannot_leave_the_folder() {
    let (dir, store) = store();
    store
        .add_note(&MemoryScope::Project("../../escape".into()), "x", 1)
        .unwrap();
    assert!(!dir.path().join("escape.json").exists());
    assert_eq!(
        store
            .notes(&MemoryScope::Project("../../escape".into()))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_broken_file_names_its_path() {
    let (_dir, store) = store();
    store.save_bot(starter_crew().remove(1)).unwrap();
    std::fs::write(store.root().join("bots").join("bolt.json"), "{ not json").unwrap();
    let err = store.bots().unwrap_err();
    assert!(
        matches!(&err, BotsError::Parse { path, .. } if path.ends_with("bolt.json")),
        "{err}"
    );
}

#[test]
fn the_file_on_disk_has_the_shape_the_docs_promise() {
    let (_dir, store) = store();
    seed_starters(&store).unwrap();
    let text = std::fs::read_to_string(store.root().join("bots").join("bolt.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["id"], "bolt");
    assert_eq!(json["harness"], "claude-code");
    assert_eq!(json["voice"], "calm-and-clear");
    assert_eq!(
        json["face"],
        serde_json::json!({ "body": "tank", "colour": "blue", "tool": "arm" })
    );
    assert_eq!(json["provider"]["service"], "anthropic");
    assert_eq!(json["version"], 1);
    let deliver =
        std::fs::read_to_string(store.root().join("playbooks").join("deliver.json")).unwrap();
    assert!(
        deliver.contains("\"asks_first\": true"),
        "the last step asks first"
    );
}

#[test]
fn a_bot_that_a_playbook_names_cannot_be_removed() {
    let (_dir, store) = store();
    seed_starters(&store).unwrap();
    let err = store.remove_bot(&id("bolt")).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("deliver")),
        "{err}"
    );
    store.remove_bot(&id("dot")).unwrap();
    assert_eq!(store.bots().unwrap().len(), 9);
}
