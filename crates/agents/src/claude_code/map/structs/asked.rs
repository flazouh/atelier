use serde_json::Value;

/// A question `claude` asked and atelier has not answered.
pub struct Asked {
    pub(in super::super) input: Value,
    pub(in super::super) suggestions: Option<Value>,
}
