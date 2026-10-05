use atelier_agents::usage::{Reading, Window};
use atelier_ui::{GaugeState, menu::Lead};

use super::super::{Vitals, traits::LoadProbe};

struct Fixed {
    cpu: f32,
}

impl LoadProbe for Fixed {
    fn cpu(&mut self) -> f32 {
        self.cpu
    }

    fn memory(&mut self) -> (u64, u64) {
        (8 << 30, 32 << 30)
    }

    fn app_memory(&mut self) -> Option<u64> {
        None
    }
}

fn reading(used: f32) -> Reading {
    Reading { windows: vec![Window { label: "5h".into(), used, resets_in: Some(60) }], note: None }
}

#[test]
fn a_sample_is_the_machine_now_with_the_processor_remembered_up_to_the_bars_it_has() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 0.4 }));
    assert!(vitals.load().is_none());
    for _ in 0..30 {
        vitals.sample();
    }
    let load = vitals.load().unwrap();
    assert_eq!((load.cpu, load.memory_used, load.memory_total), (0.4, 8 << 30, 32 << 30));
    assert_eq!(load.cpu_history.len(), 24);
}

#[test]
fn a_processor_past_all_of_it_is_held_in_range() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 3. }));
    vitals.sample();
    assert_eq!(vitals.load().unwrap().cpu, 1.);
}

#[test]
fn a_reading_replaces_the_last_in_place() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 0. }));
    vitals.settle("Claude", Lead::Monogram, Ok(reading(0.2)));
    vitals.settle("Codex", Lead::Monogram, Ok(reading(0.1)));
    vitals.settle("Claude", Lead::Monogram, Ok(reading(0.7)));
    let names: Vec<_> = vitals.providers().iter().map(|p| (p.name.to_string(), p.gauges[0].used)).collect();
    assert_eq!(names, [("Claude".to_string(), 0.7), ("Codex".to_string(), 0.1)]);
}

#[test]
fn a_provider_that_fails_keeps_its_last_numbers_told_as_old() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 0. }));
    vitals.settle("Claude", Lead::Monogram, Ok(reading(0.2)));
    vitals.settle("Claude", Lead::Monogram, Err("offline".into()));
    let claude = &vitals.providers()[0];
    assert_eq!((claude.gauges[0].used, &claude.state), (0.2, &GaugeState::Stale));
    vitals.settle("Claude", Lead::Monogram, Ok(reading(0.3)));
    assert_eq!(vitals.providers()[0].state, GaugeState::Live);
}

#[test]
fn a_provider_never_read_that_fails_does_not_show() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 0. }));
    vitals.settle("Codex", Lead::Monogram, Err("Codex did not answer".into()));
    assert!(vitals.providers().is_empty());
}

#[test]
fn a_provider_with_no_windows_that_then_fails_says_why() {
    let mut vitals = Vitals::new(Box::new(Fixed { cpu: 0. }));
    vitals.settle("OpenRouter", Lead::Monogram, Ok(Reading::default()));
    vitals.settle("OpenRouter", Lead::Monogram, Err("OpenRouter does not know this key".into()));
    assert_eq!(vitals.providers()[0].state, GaugeState::Unavailable("OpenRouter does not know this key".into()));
}

struct Says(&'static str, Result<Reading, String>);

impl atelier_agents::usage::UsageSource for Says {
    fn name(&self) -> &str {
        self.0
    }

    fn read(&self, _: &dyn atelier_project::Project, _: i64) -> Result<Reading, String> {
        self.1.clone()
    }
}

#[test]
fn every_source_is_asked_in_order_and_a_failure_does_not_stop_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let project = atelier_project::LocalProject::open(dir.path()).unwrap();
    let sources: Vec<(Lead, Box<dyn atelier_agents::usage::UsageSource>)> = vec![
        (Lead::Monogram, Box::new(Says("Claude", Err("offline".into())))),
        (Lead::Monogram, Box::new(Says("Codex", Ok(reading(0.5))))),
    ];
    let asked = Vitals::ask(&sources, &project, 0);
    assert_eq!(asked.iter().map(|(name, _, answer)| (name.as_str(), answer.is_ok())).collect::<Vec<_>>(), [("Claude", false), ("Codex", true)]);
}

#[test]
fn the_providers_asked_are_claude_and_codex_and_openrouter_only_with_a_key() {
    let names = |key: Option<&str>| Vitals::sources(key.map(String::from)).iter().map(|(_, s)| s.name().to_string()).collect::<Vec<_>>();
    assert_eq!(names(None), ["Claude", "Codex"]);
    assert_eq!(names(Some("sk-or-x")), ["Claude", "Codex", "OpenRouter"]);
}
