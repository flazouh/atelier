use atelier_bot_face::Mood;
use atelier_bots::{Harness, Provider, Voice};

use crate::bots_view::helpers::{harness_words, mood_words, provider_words, voice_words};

#[test]
fn the_words_name_the_harness_the_voice_and_the_mood_as_a_person_would_say_them() {
    assert_eq!(harness_words(Harness::ClaudeCode), "Claude Code");
    assert_eq!(voice_words(Voice::ShortAndDry), "Short and dry");
    assert_eq!(mood_words(Mood::Needs), "Needs you");
    assert_eq!(Mood::ALL.iter().map(|m| mood_words(*m)).collect::<Vec<_>>().len(), 6);
}

#[test]
fn the_provider_says_when_the_harness_picks_the_model() {
    assert_eq!(provider_words(&Provider { service: "Anthropic".into(), model: None }), "Anthropic · the harness picks the model");
    assert_eq!(
        provider_words(&Provider { service: "Anthropic".into(), model: Some("claude-opus-5-5".into()) }),
        "Anthropic · claude-opus-5-5"
    );
}
