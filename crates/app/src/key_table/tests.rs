//! One check over the whole key table: no two commands that can be active at once share a chord.
//!
//! The table has two parts. The GPUI bindings (`cx.bind_keys`, each with a context or none) and the
//! chords of beui's `keys` table, which a pane reads itself. Two GPUI bindings clash when they have the
//! same keys and different actions, and either has no context (it is live everywhere) or both have the
//! same context. A chord of the `keys` table with a modifier clashes with a GPUI binding of the same keys.
//! Bare letters of the `keys` table belong to their pane and are only checked against each other.
use std::collections::BTreeMap;

use beui::keys::{Command, Profile, chords};
use gpui_kit::TestAppContext;

/// A clash that was chosen. Each one names who wins and why the other still works.
const KNOWN: &[(&str, &str)] = &[
    // The shell's ⌘B and ⌘⇧B move the side panes. Inside the pull request view, GitQuiet's Details and
    // Files panes have the same chords, and the pull request view reads them first, on its own focus.
    // In a comment box, ⌘B is bold while the reader types.
    ("⌘b", "the pull request view (Details pane) and a comment box (bold) read it before the shell"),
    ("⌘⇧b", "the pull request view reads the Files pane before the shell"),
    // The agent panel's own close, when a session panel has the focus, beats the shell's Close tab.
    ("⌘w", "a session panel closes its own tab first; the shell closes the editor tab"),
];

fn known(keys: &str) -> bool {
    KNOWN.iter().any(|(chord, _)| glyphs(chord) == keys)
}

fn glyphs(chord: &str) -> String {
    let mut out = String::new();
    let mut rest = chord;
    let mut parts: Vec<&str> = Vec::new();
    while let Some(c) = rest.chars().next() {
        match c {
            '⌘' => parts.push(if cfg!(target_os = "macos") { "cmd" } else { "ctrl" }),
            '⇧' => parts.push("shift"),
            _ => break,
        }
        rest = &rest[c.len_utf8()..];
    }
    out.push_str(&parts.iter().map(|p| format!("{p}-")).collect::<String>());
    out.push_str(rest);
    out
}

struct Bound {
    keys: String,
    context: Option<String>,
    action: String,
}

fn bindings(cx: &mut TestAppContext) -> Vec<Bound> {
    cx.update(|cx| {
        beui::init(cx);
        crate::shell::bind_keys(cx);
        let map = cx.key_bindings();
        let map = map.borrow();
        map.bindings()
            .map(|b| Bound {
                keys: b.keystrokes().iter().map(|k| k.inner().unparse()).collect::<Vec<_>>().join(" "),
                context: b.predicate().map(|p| format!("{p:?}")),
                action: b.action().name().to_string(),
            })
            .collect()
    })
}

#[gpui_kit::test]
fn no_two_bindings_that_can_be_live_together_share_a_chord(cx: &mut TestAppContext) {
    let all = bindings(cx);
    let mut clashes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            let together = a.context.is_none() || b.context.is_none() || a.context == b.context;
            if a.keys == b.keys && a.action != b.action && together {
                clashes.entry(a.keys.clone()).or_default().push(format!(
                    "{} [{}] and {} [{}]",
                    a.action,
                    a.context.as_deref().unwrap_or("everywhere"),
                    b.action,
                    b.context.as_deref().unwrap_or("everywhere")
                ));
            }
        }
    }
    let unknown: Vec<_> = clashes.iter().filter(|(keys, _)| !known(keys)).collect();
    assert!(unknown.is_empty(), "chords in two places: {unknown:#?}");
}

/// A chord with a modifier in the `keys` table may not also be a GPUI binding of something else, in either
/// profile, unless it is on the chosen list.
#[gpui_kit::test]
fn the_keys_table_and_the_bindings_share_no_modifier_chord(cx: &mut TestAppContext) {
    let all = bindings(cx);
    let mut found = Vec::new();
    for profile in [Profile::Standard, Profile::Vim] {
        for command in Command::ALL {
            for chord in chords(profile, *command).iter().filter(|c| c.contains('⌘')) {
                let keys = glyphs(chord);
                for bound in all.iter().filter(|b| b.keys == keys) {
                    found.push((keys.clone(), format!("{command:?} ({profile:?}) and {}", bound.action)));
                }
            }
        }
    }
    assert!(!found.is_empty(), "the check finds the chords it should: ⌘B is in both tables");
    let unknown: Vec<_> = found.iter().filter(|(keys, _)| !known(keys)).collect();
    assert!(unknown.is_empty(), "chords in the keys table and in the bindings: {unknown:#?}");
}

/// Inside one profile no chord reaches two commands, and no chord is the start of another.
#[test]
fn each_profile_gives_a_chord_to_one_command_only() {
    for profile in [Profile::Standard, Profile::Vim] {
        let mut seen: BTreeMap<&str, Command> = BTreeMap::new();
        for command in Command::ALL {
            for chord in chords(profile, *command) {
                if let Some(other) = seen.insert(chord, *command) {
                    panic!("{chord} reaches {other:?} and {command:?} in {profile:?}");
                }
            }
        }
        for first in seen.keys() {
            for second in seen.keys() {
                assert!(!(second.len() > first.len() && second.starts_with(&format!("{first} "))), "{first} starts {second} in {profile:?}");
            }
        }
    }
}
