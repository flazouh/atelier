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
    /// Turns edge on and off, like blades: the part shrinks across and grows back.
    Rotor,
    /// Grows and shrinks all round, like a heart beat.
    Pulse,
}
