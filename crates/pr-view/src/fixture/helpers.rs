use atelier_forge::{ForgeError, MergeSettings, PullRef, RepoRef, Repository};

pub(super) fn missing(reference: &PullRef) -> ForgeError {
    ForgeError::NotFound(format!("{}#{}", reference.repo.slug(), reference.number))
}

/// A repository with the settings most have, for a pull request to belong to.
pub fn repository(reference: &RepoRef) -> Repository {
    Repository {
        id: "R_fixture".into(),
        reference: reference.clone(),
        url: format!("https://github.com/{}", reference.slug()),
        default_branch: Some("main".into()),
        merge: MergeSettings::default(),
        can_write: true,
    }
}
