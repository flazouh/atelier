use super::*;

thread_local! {
    static OWNED: std::cell::RefCell<Vec<tempfile::TempDir>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A settings file in a folder that goes when the test thread ends.
fn scratch(_name: &str) -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    OWNED.with(|owned| owned.borrow_mut().push(dir));
    path
}

#[test]
fn a_saved_theme_loads_back_and_a_missing_or_broken_file_is_the_default() {
    let path = scratch("theme");
    assert_eq!(load(&path), Settings::default(), "no file yet");
    update(&path, |s| s.theme = Some("Catppuccin Mocha".into())).unwrap();
    assert_eq!(load(&path).theme.as_deref(), Some("Catppuccin Mocha"));
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load(&path), Settings::default(), "a broken file falls back");
}

#[test]
fn setting_the_theme_keeps_the_recent_projects_and_unknown_keys() {
    let path = scratch("keep");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"theme":"atelier Dark","recent":[{"kind":"local","path":"/work/atelier"}],"font_size":14}"#).unwrap();
    update(&path, |s| s.theme = Some("atelier Light".into())).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let back = load(&path);
    assert_eq!(back.theme.as_deref(), Some("atelier Light"));
    assert_eq!(back.recent, [Location::Local { path: "/work/atelier".into() }]);
    assert!(text.contains("\"font_size\": 14"), "an unknown key survives: {text}");
}

#[test]
fn opening_a_project_moves_it_first_once_and_the_list_stays_short() {
    let mut s = Settings::default();
    let local = |n: usize| Location::Local { path: format!("/p/{n}").into() };
    for n in 0..12 {
        s.opened(local(n));
    }
    assert_eq!(s.recent.len(), RECENT_LIMIT);
    assert_eq!(s.recent[0], local(11));
    s.opened(local(5));
    assert_eq!(s.recent[0], local(5));
    assert_eq!(s.recent.iter().filter(|l| **l == local(5)).count(), 1);
    let remote = Location::Ssh { host: "dev-host".into(), path: "/home/user/code/atelier".into() };
    s.opened(remote.clone());
    assert_eq!(s.recent[0], remote);
    assert_eq!(remote.name(), "atelier");
    assert_eq!(remote.place(), "dev-host:/home/user/code/atelier");
}

#[test]
fn session_names_and_panels_come_back() {
    let path = scratch("panels");
    update(&path, |s| {
        s.session_names.insert("abc".into(), "Fix the flaky test".into());
        s.panels = Panels { single: true, grouped: true, widths: vec![("abc".into(), 520.)] };
    })
    .unwrap();
    let back = load(&path);
    assert_eq!(back.session_names.get("abc").map(String::as_str), Some("Fix the flaky test"));
    assert!(back.panels.single && back.panels.grouped);
    assert_eq!(back.panels.widths, [("abc".to_string(), 520.)]);
}

#[test]
fn the_mode_and_the_primary_colour_are_kept_and_an_old_file_without_them_still_loads() {
    let path = scratch("mode");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"theme": "atelier Dark", "recent": []}"#).unwrap();
    let old = load(&path);
    assert_eq!((old.mode, old.primary), (None, None));
    update(&path, |s| {
        s.mode = Some("system".into());
        s.primary = Some([2, 133, 247]);
    })
    .unwrap();
    let back = load(&path);
    assert_eq!((back.mode.as_deref(), back.primary, back.theme.as_deref()), (Some("system"), Some([2, 133, 247]), Some("atelier Dark")));
    update(&path, |s| s.primary = None).unwrap();
    assert_eq!(load(&path).primary, None, "back to the default");
}
/// The sessions open at quit, and the one in front, come back for the next launch.
#[test]
fn the_open_sessions_come_back() {
    let path = scratch("open");
    let here = Location::Local { path: "/work/atelier".into() };
    update(&path, |s| {
        s.open = vec![
            OpenSession { location: here.clone(), id: "s1".into(), title: "Fix the lease".into(), agent: None, provider: None },
            OpenSession { location: here.clone(), id: "s2".into(), title: "Add a test".into(), agent: None, provider: None },
        ];
        s.front = Some("s2".into());
    })
    .unwrap();
    let back = load(&path);
    assert_eq!(back.open.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), ["s1", "s2"]);
    assert_eq!(back.open[0].location, here);
    assert_eq!(back.front.as_deref(), Some("s2"));
}

/// Each open session keeps the agent that runs it; a file from before has none, and its sessions take the default.
#[test]
fn an_open_session_keeps_its_agent_and_an_old_file_has_none() {
    let here = Location::Local { path: "/work/atelier".into() };
    let kept = OpenSession { location: here, id: "s1".into(), title: "hi".into(), agent: Some("cursor".into()), provider: Some("openrouter".into()) };
    let round: OpenSession = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round, kept);
    let old: OpenSession = serde_json::from_str(r#"{"location":{"kind":"local","path":"/w"},"id":"s1","title":"hi"}"#).unwrap();
    assert_eq!(old.agent, None);
}

#[test]
fn the_task_rules_the_reader_turned_off_are_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{\"theme\":\"atelier Light\"}").unwrap();
    assert!(old.task_rules_off.is_empty(), "a file from before the rules loads");
    let kept = Settings { task_rules_off: vec!["merge".into()], ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.task_rules_off, ["merge"]);
}

#[test]
fn the_design_choice_is_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.design_tabs, None);
    let kept = Settings { design_tabs: Some(1), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.design_tabs, Some(1));
    // A file an older atelier wrote, with the toggle choice in it, still reads and keeps the key as it is.
    let older: Settings = serde_json::from_str(r#"{"design_toggle": 3, "design_tabs": 2}"#).unwrap();
    assert_eq!(older.design_tabs, Some(2));
    assert_eq!(older.other.get("design_toggle"), Some(&serde_json::json!(3)));
}

#[test]
fn the_elevation_choice_is_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.design_elevation, None);
    let kept = Settings { design_elevation: Some(3), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.design_elevation, Some(3));
}

#[test]
fn the_elevation_strength_is_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.design_strength, None);
    let kept = Settings { design_strength: Some(70), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.design_strength, Some(70));
}

#[test]
fn a_project_keeps_its_badge_colour_and_icon_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert!(old.project_colors.is_empty() && old.project_icons.is_empty());
    let mut kept = Settings::default();
    kept.project_colors.insert("~/code/x".into(), 7);
    kept.project_icons.insert("~/code/x".into(), "/data/project-icons/ab.svg".into());
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.project_colors["~/code/x"], 7);
    assert_eq!(round.project_icons["~/code/x"], "/data/project-icons/ab.svg");
}

/// The app saves from a thread for each change, so changes made together are all kept.
#[test]
fn saves_made_together_all_keep_their_change() {
    for round in 0..20 {
        let path = scratch("together");
        let start = std::sync::Arc::new(std::sync::Barrier::new(16));
        let threads: Vec<_> = (0..16)
            .map(|i| {
                let (path, start) = (path.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    update(&path, |s| s.archived_sessions.push(format!("s{i}")))
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap().unwrap_or_else(|e| panic!("round {round}: {e}"));
        }
        let mut kept = load(&path).archived_sessions;
        kept.sort();
        let mut want: Vec<_> = (0..16).map(|i| format!("s{i}")).collect();
        want.sort();
        assert_eq!(kept, want, "round {round}");
    }
}

#[test]
fn the_language_is_kept_by_its_tag() {
    let file = scratch("language");
    update(&file, |s| s.language = Some("pt-BR".into())).unwrap();
    assert_eq!(load(&file).language.as_deref(), Some("pt-BR"));
    update(&file, |s| s.language = None).unwrap();
    assert_eq!(load(&file).language, None, "unset follows the system");
}

/// A session with no provider writes none, so a file from before the provider was kept reads the same way.
#[test]
fn an_open_session_with_no_provider_writes_none_and_reads_back() {
    let here = Location::Local { path: "/p".into() };
    let saved = OpenSession { location: here, id: "s1".into(), title: "hi".into(), agent: None, provider: None };
    let text = serde_json::to_string(&saved).unwrap();
    assert!(!text.contains("provider"), "{text}");
    assert_eq!(serde_json::from_str::<OpenSession>(&text).unwrap(), saved);
}

fn pairs(of: &[(&str, &str)]) -> Vec<(String, String)> {
    of.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn the_models_follow_the_readers_order_then_the_agents() {
    let prefs = AgentModels { order: vec!["c".into(), "a".into(), "gone".into()], ..AgentModels::default() };
    let list = prefs.arranged(&pairs(&[("a", "A"), ("b", "B"), ("c", "C")]));
    assert_eq!(list, pairs(&[("c", "C"), ("a", "A"), ("b", "B")]), "c and a first; b after; an id the agent dropped is skipped");
}

#[test]
fn what_the_agent_reported_replaces_its_built_in_list() {
    let prefs = AgentModels { known: pairs(&[("claude-opus-5-5", "Opus 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5")]), ..AgentModels::default() };
    assert_eq!(prefs.arranged(&pairs(&[("opus", "Opus")])), prefs.known, "the three plain names give way to the real ones");
    assert_eq!(AgentModels::default().arranged(&pairs(&[("opus", "Opus")])), pairs(&[("opus", "Opus")]), "until it has reported");
}

#[test]
fn a_default_that_the_list_no_longer_has_is_none() {
    let list = pairs(&[("a", "A"), ("b", "B")]);
    assert_eq!(AgentModels { default: Some("b".into()), ..AgentModels::default() }.default_in(&list), Some("b".into()));
    assert_eq!(AgentModels { default: Some("z".into()), ..AgentModels::default() }.default_in(&list), None);
}

#[test]
fn the_models_survive_a_save_and_an_older_file_has_none() {
    let mut settings = Settings::default();
    settings.agent_models.insert("claude-code".into(), AgentModels { default: Some("x".into()), order: vec!["x".into()], known: pairs(&[("x", "X")]), hidden: vec!["y".into()] });
    let back: Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(back.agent_models, settings.agent_models);
    assert!(serde_json::from_str::<Settings>("{}").unwrap().agent_models.is_empty());
}

#[test]
fn the_changelog_of_a_downloaded_update_is_kept_and_other_keys_stay() {
    let text = r#"{"whats_new":{"version":"0.1.4","notes":"- **A:** b"},"future_key":1}"#;
    let settings: Settings = serde_json::from_str(text).unwrap();
    assert_eq!(settings.whats_new, Some(WhatsNew { version: "0.1.4".into(), notes: "- **A:** b".into() }));
    let again: serde_json::Value = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(again["whats_new"]["version"], "0.1.4");
    assert_eq!(again["future_key"], 1, "a key this version does not know is kept");
    assert_eq!(Settings::default().whats_new, None);
}

#[test]
fn a_hidden_model_leaves_the_picker_but_the_default_and_the_last_one_stay() {
    let list = pairs(&[("a", "A"), ("b", "B"), ("c", "C")]);
    let prefs = AgentModels { hidden: vec!["b".into(), "c".into()], default: Some("c".into()), ..AgentModels::default() };
    assert_eq!(prefs.visible(&list), pairs(&[("a", "A"), ("c", "C")]), "b is hidden, the default c is not");
    let all_hidden = AgentModels { hidden: vec!["a".into(), "b".into(), "c".into()], ..AgentModels::default() };
    assert_eq!(all_hidden.visible(&list), list, "a picker is never empty");
}
#[test]
fn agent_tools_are_on_unless_the_file_turns_them_off() {
    assert_eq!(Settings::default().capabilities.agent_tools, None);
    let path = scratch("tools");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"capabilities":{"agent_tools":false}}"#).unwrap();
    assert_eq!(load(&path).capabilities.agent_tools, Some(false));
    update(&path, |s| s.theme = Some("x".into())).unwrap();
    assert_eq!(load(&path).capabilities.agent_tools, Some(false), "another save keeps it");
}

#[test]
fn the_connected_accounts_round_trip_and_the_file_holds_no_key() {
    let path = scratch("accounts");
    assert_eq!(Settings::default().accounts, AccountsSaved::default());
    let key = "lin_api_SECRET_SECRET";
    update(&path, |s| {
        s.accounts.linear = Some(LinearSaved { person: Some("Ada".into()) });
        s.accounts.github_issues = Some(GithubIssuesSaved { repo: "acme/web".into(), person: None });
    })
    .unwrap();
    let back = load(&path);
    assert_eq!(back.accounts.linear, Some(LinearSaved { person: Some("Ada".into()) }));
    assert_eq!(back.accounts.github_issues.map(|g| g.repo), Some("acme/web".into()));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains(key), "{text}");
    update(&path, |s| s.accounts = AccountsSaved::default()).unwrap();
    assert_eq!(load(&path).accounts, AccountsSaved::default(), "forgetting clears both");
}

#[test]
fn a_save_to_a_path_given_on_purpose_does_not_make_its_folder() {
    let path = scratch("gone");
    let dir = path.parent().unwrap().to_path_buf();
    std::fs::remove_dir_all(&dir).unwrap();
    let error = update(&path, |s| s.theme = Some("atelier Light".into())).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "a folder that is gone stays gone");
    assert!(!dir.exists(), "the save did not make the folder again");
}
