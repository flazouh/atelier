use std::sync::Arc;

use atelier_ui::{AgentLook, BrandMark};

use crate::{labs::Lab, session::Backend};

#[derive(Clone)]
pub struct Agent {
    pub backend: Arc<dyn Backend>,
    /// What a person calls it, for a session row or a menu.
    pub name: &'static str,
    /// Its mark, or `None` for a monogram.
    pub mark: Option<BrandMark>,
    pub look: AgentLook,
    /// The lab whose models it runs, for the marks beside them.
    pub lab: Lab,
}
