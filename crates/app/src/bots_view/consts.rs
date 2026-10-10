/// The face data the player draws from, built into the app.
pub const FACE_DATA: &str = include_str!("../../../../docs/bots/faces.v1.json");
/// The side of a face in a sidebar row, in design pixels.
pub const ROW_FACE: f32 = 22.;
/// The side of the face in the profile, in design pixels.
pub const PROFILE_FACE: f32 = 168.;
/// The room the profile leaves round its content, in design pixels.
pub const DETAIL_PAD: f32 = 24.;
/// The folder, next to the settings file, that keeps the bots.
pub const FOLDER: &str = "bots";
/// What the profile says while the folder is read, when it has no folder, and when the folder holds no bot.
pub const READING: &str = "Reading the bots…";
pub const NO_FOLDER: &str = "There is no folder to keep bots in.";
pub const NOTHING: &str = "No bots yet.";
/// What the notes section says when the bot remembers nothing yet.
pub const NO_NOTES: &str = "No notes yet.";
