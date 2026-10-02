use atelier_forge::{Forge, ForgeError, Involved};

pub(super) fn read_involved(forge: &dyn Forge, scope: Option<&atelier_forge::RepoRef>) -> Result<Vec<Involved>, ForgeError> {
    match scope {
        Some(repo) => forge.involved_in(repo),
        None => forge.involved(),
    }
}
