use atelier_ui::merge::MergeMethod as UiMethod;

pub(super) fn verb(method: UiMethod) -> &'static str {
    match method {
        UiMethod::Merge => "Merge",
        UiMethod::Squash => "Squash and merge",
        UiMethod::Rebase => "Rebase and merge",
    }
}
