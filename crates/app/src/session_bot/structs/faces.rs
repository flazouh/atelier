use std::rc::Rc;

use atelier_bot_face::FaceSet;
use gpui_kit::Global;

use crate::bots_view::consts::FACE_DATA;

/// The face data built into the app, read once for every session's face.
pub struct Faces(pub Rc<FaceSet>);

impl Default for Faces {
    fn default() -> Self {
        Self(Rc::new(FaceSet::from_json(FACE_DATA).expect("the face data built into the app loads")))
    }
}

impl Global for Faces {}
