use std::path::Path;

use lsp_types::{GotoDefinitionResponse, Location, LocationLink, Position};

use super::structs::Target;

/// Every answer to `textDocument/definition` as links, the one shape an editor needs. A plain
/// location has no separate name range, so its whole range serves as both.
pub fn definition_links(answer: Option<GotoDefinitionResponse>) -> Vec<LocationLink> {
    match answer {
        None => vec![],
        Some(GotoDefinitionResponse::Scalar(location)) => vec![location_link(location)],
        Some(GotoDefinitionResponse::Array(locations)) => locations.into_iter().map(location_link).collect(),
        Some(GotoDefinitionResponse::Link(links)) => links,
    }
}

/// A plain location as a link that selects its range.
pub fn location_link(location: Location) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: location.uri,
        target_range: location.range,
        target_selection_range: location.range,
    }
}

/// Whether a definition answer has nowhere to go: it is empty, or every target is the symbol at
/// `position` in the file `here`. That is the caret already on a declaration, and the answer Zed
/// gives there is the symbol's uses.
pub fn lands_on_itself(targets: &[Target], here: &Path, position: Position) -> bool {
    targets.iter().all(|target| {
        target.path.as_deref() == Some(here) && target.range.start <= position && position <= target.range.end
    })
}

/// Targets in reading order: by file, then by position.
pub fn sort_targets(targets: &mut [Target]) {
    targets.sort_by(|a, b| (a.uri.as_str(), a.range.start).cmp(&(b.uri.as_str(), b.range.start)));
}
