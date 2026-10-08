use atelier_agents::registry;
use gpui_kit::TestAppContext;
use atelier_settings::AgentModels;

use super::{ModelPrefs, offered, start_model};

fn claude() -> registry::Agent {
    registry::by_backend("claude-code").expect("Claude Code is an agent")
}

fn with(prefs: AgentModels, cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut all = ModelPrefs::default();
        all.0.insert("claude-code".into(), prefs);
        cx.set_global(all);
    });
}

/// Before the agent has reported, its three plain names are the list and none is named for a new session.
#[gpui_kit::test]
fn the_built_in_list_names_no_start_model(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_global(ModelPrefs::default()));
    let (list, start) = cx.update(|cx| (offered(&claude(), cx), start_model(&claude(), cx)));
    assert_eq!(list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["opus", "sonnet", "haiku"]);
    assert_eq!(start, None);
}

/// What the agent reported replaces the plain names, in the reader's order; a new session starts on the default, else on the first.
#[gpui_kit::test]
fn a_reported_list_is_ordered_and_the_default_starts_new_sessions(cx: &mut TestAppContext) {
    let known = vec![("claude-opus-5-5".to_string(), "Opus 5.5".to_string()), ("claude-sonnet-5-5".to_string(), "Sonnet 5.5".to_string())];
    with(AgentModels { known: known.clone(), ..AgentModels::default() }, cx);
    assert_eq!(cx.update(|cx| start_model(&claude(), cx)), Some("claude-opus-5-5".into()), "the first, with no default");
    with(AgentModels { known, default: Some("claude-sonnet-5-5".into()), order: vec!["claude-sonnet-5-5".into()], ..AgentModels::default() }, cx);
    let (list, start) = cx.update(|cx| (offered(&claude(), cx), start_model(&claude(), cx)));
    assert_eq!(list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-sonnet-5-5", "claude-opus-5-5"]);
    assert_eq!(start, Some("claude-sonnet-5-5".into()));
}

/// A hidden model leaves the picker's list and stays in Settings' list; starring a hidden one brings it back.
#[gpui_kit::test]
fn a_hidden_model_is_in_settings_and_not_in_the_picker(cx: &mut TestAppContext) {
    let known = vec![("claude-opus-5-5".to_string(), "Opus 5.5".to_string()), ("claude-sonnet-5-5".to_string(), "Sonnet 5.5".to_string())];
    cx.update(|cx| {
        let mut all = ModelPrefs::default();
        all.0.insert("claude-code".into(), AgentModels { known, ..AgentModels::default() });
        cx.set_global(all);
        super::set_hidden("claude-code", "claude-sonnet-5-5", true, cx);
    });
    let (picker, settings) = cx.update(|cx| (offered(&claude(), cx), super::all(&claude(), cx)));
    assert_eq!(picker.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5"], "the picker leaves it out");
    assert_eq!(settings.iter().map(|(m, hidden)| (m.id.as_str(), *hidden)).collect::<Vec<_>>(), [("claude-opus-5-5", false), ("claude-sonnet-5-5", true)]);
    cx.update(|cx| super::choose_default("claude-code", "claude-sonnet-5-5", cx));
    let picker = cx.update(|cx| offered(&claude(), cx));
    assert_eq!(picker.len(), 2, "starring it brings it back");
}

/// A drag in the picker lists the visible models only: the hidden ones keep their places among the others in Settings.
#[gpui_kit::test]
fn dragging_the_visible_models_leaves_the_hidden_where_they_were(cx: &mut TestAppContext) {
    let known: Vec<(String, String)> =
        ["a", "b", "c", "d"].iter().map(|id| (format!("claude-{id}"), id.to_uppercase())).collect();
    cx.update(|cx| {
        let mut all = ModelPrefs::default();
        all.0.insert("claude-code".into(), AgentModels { known, hidden: vec!["claude-b".into()], ..AgentModels::default() });
        cx.set_global(all);
        // The picker lists a, c, d; the reader drags d to the top.
        super::reorder_visible(&claude(), vec!["claude-d".into(), "claude-a".into(), "claude-c".into()], cx);
    });
    let settings: Vec<String> = cx.update(|cx| super::all(&claude(), cx).into_iter().map(|(m, _)| m.id).collect());
    assert_eq!(settings, ["claude-d", "claude-b", "claude-a", "claude-c"], "b stays in the second place, which was b's");
}
