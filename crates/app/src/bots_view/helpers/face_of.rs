use atelier_bot_face::FaceSet;
use atelier_bots::{Body, Bot};

/// The face a bot is drawn with, as an index into the set: the one named by its id, else the starter with the same
/// body, so a bot made later still has a face until the builder draws its own.
pub fn face_of(set: &FaceSet, bot: &Bot) -> Option<usize> {
    let at = |id: &str| set.bots.iter().position(|m| m.id == id);
    at(bot.id.as_str()).or_else(|| at(starter_with(bot.face.body)))
}

/// The starter whose body a bot with no face of its own borrows.
fn starter_with(body: Body) -> &'static str {
    match body {
        Body::Tank => "bolt",
        Body::Wheels => "pip",
        Body::Spring => "olive",
        Body::Hover => "skip",
        Body::Legs => "dot",
        Body::Feet => "nimbus",
        Body::Ball => "keyla",
    }
}
