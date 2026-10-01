use atelier_pr_view::{
    fixture::{big::Big, relay::Relay},
    };

/// The repository the story reads, kept on disk while the story shows it.
#[allow(dead_code)]
pub(super) enum Keep {
    _Relay(Relay),
    _Big(Big),
    _Real,
}
