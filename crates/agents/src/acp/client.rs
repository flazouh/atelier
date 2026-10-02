//! What atelier asks of an ACP agent, as a method and its params. `protocol` numbers the requests and
//! frames them with `rpc`.

mod helpers;
mod structs;
mod types;

pub(super) use helpers::{
    authenticate, cancel, initialize, list_sessions, load_session, new_session,
    permission_cancelled, permission_selected, prompt, set_config_option, set_mode, set_model,
};
pub(super) use structs::Outgoing;
