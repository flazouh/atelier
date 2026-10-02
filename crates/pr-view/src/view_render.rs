//! How the pull request view draws: a rail on the left, and on the right the seen bar, the tree and the
//! diff. The layout is the pull request story's (see [`crate::layout`]); the data is the model's.

mod helpers;
mod impls;
mod types;

#[cfg(test)]
pub(crate) use helpers::body_needs_fold;

#[cfg(test)]
mod fold_tests {
    use super::body_needs_fold;

    #[test]
    fn a_short_description_needs_no_fold_button_and_a_long_one_does() {
        assert!(!body_needs_fold("A QA pull request for the atelier pull request view. It adds add, sub and div, and a README line. Safe to close."));
        assert!(body_needs_fold(&"A line of the description.\n".repeat(7)));
        assert!(body_needs_fold(&"word ".repeat(200)));
    }
}
