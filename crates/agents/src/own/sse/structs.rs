use super::types::MAX_LINE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    pub name: Option<String>,
    pub data: String,
}

#[derive(Default)]
pub struct Parser {
    pub(super) line: Vec<u8>,
    name: Option<String>,
    pub(super) data: String,
    has_data: bool,
}

impl Parser {
    /// Feeds bytes; calls `out` for each event they complete.
    pub fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(SseEvent)) -> Result<(), String> {
        let mut rest = bytes;
        while let Some(end) = rest.iter().position(|b| *b == b'\n') {
            self.line.extend_from_slice(&rest[..end]);
            rest = &rest[end + 1..];
            self.take_line(out);
        }
        self.line.extend_from_slice(rest);
        if self.line.len() > MAX_LINE {
            return Err("a line of the stream is too long".into());
        }
        Ok(())
    }

    /// The stream ended: a last event with no blank line after it still counts.
    pub fn finish(&mut self, out: &mut dyn FnMut(SseEvent)) {
        if !self.line.is_empty() {
            self.take_line(out);
        }
        self.dispatch(out);
    }

    fn take_line(&mut self, out: &mut dyn FnMut(SseEvent)) {
        let mut line = std::mem::take(&mut self.line);
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line[0] == b':' {
            return;
        }
        let text = String::from_utf8_lossy(&line);
        let (field, value) = match text.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (text.as_ref(), ""),
        };
        match field {
            "event" => self.name = Some(value.to_string()),
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            _ => {}
        }
        // Give the buffer back, empty, so the next line reuses it.
        line.clear();
        self.line = line;
    }

    fn dispatch(&mut self, out: &mut dyn FnMut(SseEvent)) {
        if self.has_data {
            out(SseEvent { name: self.name.take(), data: std::mem::take(&mut self.data) });
        } else {
            self.name = None;
        }
        self.has_data = false;
    }
}
