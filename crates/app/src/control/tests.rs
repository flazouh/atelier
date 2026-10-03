use gpui_kit::TestAppContext;

use std::path::Path;

use super::{helpers::{chosen, session_json}, types::Request};
use crate::fake_agent::start;

#[test]
fn a_line_names_its_command() {
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"state"}"#).unwrap(), Request::State);
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"new_session"}"#).unwrap(), Request::NewSession { agent: None });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"new_session","agent":"Cursor"}"#).unwrap(), Request::NewSession { agent: Some("Cursor".into()) });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"send","text":"hi"}"#).unwrap(), Request::Send { text: "hi".into() });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"limit"}"#).unwrap(), Request::Limit);
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"find","name":"a"}"#).unwrap(), Request::Find { name: "a".into() });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"click","name":"a"}"#).unwrap(), Request::Click { name: Some("a".into()), x: None, y: None });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"click","x":1,"y":2.5}"#).unwrap(), Request::Click { name: None, x: Some(1.), y: Some(2.5) });
    assert_eq!(serde_json::from_str::<Request>(r#"{"cmd":"open","path":"~/p","host":"h"}"#).unwrap(), Request::Open { path: "~/p".into(), host: Some("h".into()) });
    assert!(serde_json::from_str::<Request>(r#"{"cmd":"explode"}"#).is_err());
    assert!(serde_json::from_str::<Request>(r#"{"cmd":"send"}"#).is_err(), "a message needs its text");
}

/// What a person would see after a send, as a script reads it: the message, then the agent's waiting words.
#[gpui_kit::test]
fn the_state_of_a_session_that_waits_lists_the_message_and_the_waiting_line(cx: &mut TestAppContext) {
    let (session, _fake, cx) = start(cx, vec![], false);
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("hello".into(), cx)));
    let json = cx.update(|_, cx| session_json(session.read(cx), true));
    assert_eq!(json["working"], true);
    assert_eq!(json["front"], true);
    let rows = json["rows"].as_array().unwrap();
    assert_eq!(rows[0]["kind"], "user");
    assert_eq!(rows[0]["text"], "hello");
    assert_eq!(rows[1]["kind"], "waiting");
    assert!(!rows[1]["label"].as_str().unwrap().is_empty());
}

/// A debug build listens without being asked, a release build only when asked, and "off" wins.
#[test]
fn a_debug_build_listens_by_default_and_a_release_build_when_asked() {
    let run = Path::new("/run/user/1");
    assert_eq!(chosen(None, true, run, 7), Some("/run/user/1/atelier-7.sock".into()));
    assert_eq!(chosen(None, false, run, 7), None);
    assert_eq!(chosen(Some("/tmp/x.sock".into()), false, run, 7), Some("/tmp/x.sock".into()));
    assert_eq!(chosen(Some("/tmp/x.sock".into()), true, run, 7), Some("/tmp/x.sock".into()));
    assert_eq!(chosen(Some("off".into()), true, run, 7), None);
    assert_eq!(chosen(Some("".into()), true, run, 7), Some("/run/user/1/atelier-7.sock".into()), "empty is unset");
}
