use serde::Deserialize;

/// The kind of movement a part makes by habit.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HabitKind {
    Swing,
    Spin,
    Sweep,
    Spring,
    Flicker,
    Bob,
    Scuttle,
    Wave,
}
