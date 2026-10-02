use gpui_kit::SharedString;

/// The chips' names for tests and screenshots, by their place in the list.
pub(super) const HOST_CHIPS: [&str; 8] = ["ssh-host-0", "ssh-host-1", "ssh-host-2", "ssh-host-3", "ssh-host-4", "ssh-host-5", "ssh-host-6", "ssh-host-7"];

#[derive(Clone, Debug, PartialEq)]
pub enum SshFormEvent {
    Connect { host: String },
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// Connecting: the step it is on.
    Connecting(SharedString),
    Failed(SharedString),
}
