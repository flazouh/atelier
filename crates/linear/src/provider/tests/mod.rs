//! Tests of the Linear provider. The fixtures are real, read-only answers of Linear with every id, name, title and
//! address replaced by a neutral one. `server` replays them over a socket, so the HTTP paths run for real.
mod live;
mod mapping;
mod replay;
mod server;

use serde_json::Value;

/// One recorded answer of Linear, by file name.
pub fn fixture(name: &str) -> Value {
    let text = match name {
        "viewer" => include_str!("../fixtures/viewer.json"),
        "teams" => include_str!("../fixtures/teams.json"),
        "list" => include_str!("../fixtures/list.json"),
        "list_page2" => include_str!("../fixtures/list_page2.json"),
        "list_filtered" => include_str!("../fixtures/list_filtered.json"),
        "list_text" => include_str!("../fixtures/list_text.json"),
        "get" => include_str!("../fixtures/get.json"),
        "get_missing" => include_str!("../fixtures/get_missing.json"),
        "labels" => include_str!("../fixtures/labels.json"),
        "projects" => include_str!("../fixtures/projects.json"),
        "states" => include_str!("../fixtures/states.json"),
        "comments" => include_str!("../fixtures/comments.json"),
        "history" => include_str!("../fixtures/history.json"),
        other => panic!("no fixture named {other}"),
    };
    serde_json::from_str(text).expect("a fixture is JSON")
}
