//! A draft is one request with no tools and no record: the model's text, trimmed.
use std::sync::{Arc, Mutex};

use crate::{
    own::{
        OwnAgent, OwnOptions,
        message::{Block, Cancel, Delta, Model, ModelError, ModelRequest, Reply, StopReason, TokenUsage},
    },
    session::Backend,
};

/// A model that answers every request with the same words, and keeps what it was asked.
struct Canned {
    asked: Mutex<Vec<(String, usize, String)>>,
}

impl Model for Canned {
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), _: &Cancel) -> Result<Reply, ModelError> {
        let prompt = match request.messages.first().and_then(|m| m.blocks.first()) {
            Some(Block::Text { text }) => text.clone(),
            _ => String::new(),
        };
        self.asked.lock().unwrap().push((request.model.to_string(), request.tools.len(), prompt));
        sink(Delta::Text(" Release the lease ".into()));
        sink(Delta::Text("on exit\n".into()));
        Ok(Reply { blocks: vec![], stop: StopReason::EndTurn, usage: TokenUsage::default(), malformed: vec![] })
    }
}

#[test]
fn a_draft_is_one_request_with_no_tools() {
    let model = Arc::new(Canned { asked: Mutex::default() });
    let agent = OwnAgent::new(model.clone(), OwnOptions::default());
    let project = lathe_project::LocalProject::open(tempfile::tempdir().unwrap().path()).unwrap();
    assert_eq!(agent.draft(&project, "Name this branch.", Some("m")).unwrap(), "Release the lease on exit");
    assert_eq!(*model.asked.lock().unwrap(), [("m".to_string(), 0, "Name this branch.".to_string())]);
}
