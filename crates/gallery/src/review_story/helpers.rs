use gpui_kit::{AnyElement, Entity, IntoElement};

use super::structs::ReviewStory;

/// So the gallery can hold the story as any other element.
pub fn element(story: &Entity<ReviewStory>) -> AnyElement {
    story.clone().into_any_element()
}
