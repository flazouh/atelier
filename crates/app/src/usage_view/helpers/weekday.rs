use atelier_agents::usage_history::Day;
/// The short name of the day of the week.
pub fn weekday(day: Day) -> &'static str {
    // 1970-01-01 was a Thursday.
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    NAMES[(day.epoch_days() + 4).rem_euclid(7) as usize]
}
