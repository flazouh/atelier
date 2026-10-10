use atelier_bots::{BotStore, DiskBotStore, MemoryScope};

use crate::bots_view::helpers::read_library;

#[test]
fn an_empty_folder_is_seeded_with_the_ten_starters_and_read_in_the_order_of_their_ids() {
    let dir = tempfile::tempdir().unwrap();
    let library = read_library(dir.path());
    assert_eq!(library.error, None);
    let ids: Vec<&str> = library.entries.iter().map(|e| e.bot.id.as_str()).collect();
    assert_eq!(ids, ["bolt", "dot", "gus", "ink", "mimi", "nimbus", "olive", "pip", "quill", "skip"]);
    assert_eq!(std::fs::read_dir(dir.path().join("bots")).unwrap().count(), 10, "one file per bot");
    assert!(library.entries.iter().all(|e| e.notes.is_empty()), "a new bot remembers nothing");
    // A second read seeds nothing more.
    assert_eq!(read_library(dir.path()).entries.len(), 10);
}

#[test]
fn a_bot_carries_the_notes_of_its_own_layer() {
    let dir = tempfile::tempdir().unwrap();
    let store = DiskBotStore::new(dir.path());
    atelier_bots::seed_starters(&store).unwrap();
    store.add_note(&MemoryScope::Bot("dot".parse().unwrap()), "Read the failing test first.", 1_000).unwrap();
    let library = read_library(dir.path());
    let dot = library.entries.iter().find(|e| e.bot.id.as_str() == "dot").unwrap();
    assert_eq!(dot.notes.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Read the failing test first."]);
}

#[test]
fn a_file_that_does_not_parse_is_an_error_that_names_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bots")).unwrap();
    std::fs::write(dir.path().join("bots/bad.json"), "{").unwrap();
    let library = read_library(dir.path());
    let error = library.error.expect("the broken file is an error");
    assert!(error.contains("bad.json"), "the error names the file: {error}");
    assert!(library.entries.is_empty());
}
