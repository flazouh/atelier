/// One request: the verb, the path, the headers (names in lower case) and the body.
#[derive(Debug, Default)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.trim())
    }
}

/// One answer. The connection closes after it.
#[derive(Debug)]
pub(crate) struct Response {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "application/json".into())],
            body: body.to_string().into_bytes(),
        }
    }

    pub fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}
