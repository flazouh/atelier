use serde_json::Value;

/// The text fields of a tool call's input, read from the start of its JSON while it still streams in: a value that has closed
/// whole, the last one cut where the stream stopped. Only the outermost object's string values count; the others (a flag, a
/// list) wait for the whole input. `None` until a field has begun.
pub(crate) fn fields(json: &str) -> Option<Value> {
    let (fields, _) = read(json);
    (!fields.is_empty()).then_some(Value::Object(fields))
}

/// As [`fields`], with only the strings that have closed: a field the stream cut is left out, so a path is never half a path.
pub(crate) fn closed(json: &str) -> Option<Value> {
    let (mut fields, cut) = read(json);
    if let Some(cut) = cut {
        fields.remove(&cut);
    }
    (!fields.is_empty()).then_some(Value::Object(fields))
}

/// The string fields of `json` so far, and the key of the one the stream cut, if it did.
fn read(json: &str) -> (serde_json::Map<String, Value>, Option<String>) {
    let bytes = json.as_bytes();
    let skip = |mut i: usize| {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        i
    };
    let mut i = skip(0);
    if bytes.get(i) != Some(&b'{') {
        return (serde_json::Map::new(), None);
    }
    i += 1;
    let (mut fields, mut cut) = (serde_json::Map::new(), None);
    loop {
        i = skip(i);
        if bytes.get(i) != Some(&b'"') {
            break;
        }
        let Some(end) = string_end(bytes, i) else { break };
        let Ok(key) = serde_json::from_str::<String>(&json[i..=end]) else { break };
        i = end + 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            break;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => break,
            Some(b'"') => match string_end(bytes, i) {
                Some(end) => {
                    let Ok(text) = serde_json::from_str::<String>(&json[i..=end]) else { break };
                    fields.insert(key, Value::String(text));
                    i = end + 1;
                }
                None => {
                    fields.insert(key.clone(), Value::String(cut_string(&json[i + 1..])));
                    cut = Some(key);
                    break;
                }
            },
            Some(_) => match value_end(bytes, i) {
                Some(end) => i = end,
                None => break,
            },
        }
    }
    (fields, cut)
}

/// The end of the value that is not a string and starts at `start`: the `,` or `}` that follows it, or `None` while it is open.
fn value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let (mut depth, mut i) = (0usize, start);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = string_end(bytes, i)?,
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth == 0 => return Some(i),
            b'}' | b']' => depth -= 1,
            b',' if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The text of a JSON string whose closing quote has not come: `raw` with a cut escape left off, unescaped.
fn cut_string(raw: &str) -> String {
    let mut raw = raw;
    // A backslash that ends the text starts an escape that has not arrived.
    let slashes = raw.bytes().rev().take_while(|&b| b == b'\\').count();
    if slashes % 2 == 1 {
        raw = &raw[..raw.len() - 1];
    }
    // So does a `\u` with fewer than four digits after it.
    if let Some(at) = raw.rfind("\\u")
        && raw[..at].bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 0
        && raw[at + 2..].len() < 4
    {
        raw = &raw[..at];
    }
    // A high surrogate waits for its pair.
    loop {
        if let Ok(text) = serde_json::from_str::<String>(&format!("\"{raw}\"")) {
            return text;
        }
        match raw.len().checked_sub(6) {
            Some(at) if raw.is_char_boundary(at) && raw[at..].starts_with("\\u") => raw = &raw[..at],
            _ => return String::new(),
        }
    }
}

/// The index of the quote that closes the string opening at `start`, or `None` while it is still open.
pub(crate) fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'"' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}
