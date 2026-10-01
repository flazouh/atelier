use serde_json::Value;

use crate::{ForgeError, ForgeResult, PullRef};

pub(super) fn decode<T: serde::de::DeserializeOwned>(value: Value) -> ForgeResult<T> {
    serde_json::from_value(value).map_err(|e| ForgeError::Unexpected(format!("the answer has an unexpected shape: {e}")))
}

pub(super) fn not_found(what: &str) -> ForgeError {
    ForgeError::NotFound(what.to_string())
}

/// One field of the pull request in a query's data, taken out of it.
pub(super) fn pull_field<T: serde::de::DeserializeOwned>(data: &mut Value, field: &str, reference: &PullRef) -> ForgeResult<T> {
    let pull = &mut data["repository"]["pullRequest"];
    if pull.is_null() {
        return Err(not_found(&format!("{}#{}", reference.repo.slug(), reference.number)));
    }
    decode(pull[field].take())
}
