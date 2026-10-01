//! No story names a colour: every one comes from the theme in force. This reads the gallery's sources,
//! code only (comments dropped), and fails on a hex colour, an `rgb(`/`rgba(` call or a named white or
//! black. Tests may hold colours.

use std::path::Path;


fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                sources(&path, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(path);
        }
    }
}

fn is_colour(code: &str) -> Option<&'static str> {
    let bytes = code.as_bytes();
    for (i, _) in code.match_indices("0x") {
        let digits = bytes[i + 2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        if digits == 6 || digits == 8 {
            return Some("a hex number");
        }
    }
    for (i, _) in code.match_indices("\"#") {
        let digits = bytes[i + 2..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
        if matches!(digits, 3 | 4 | 6 | 8) && bytes.get(i + 2 + digits) == Some(&b'"') {
            return Some("a hex string");
        }
    }
    // Whole words only: `to_rgb(` converts a colour and `transparent_black()` is no fill at all.
    let called = |call: &str| {
        code.match_indices(call).any(|(i, _)| i == 0 || !matches!(bytes[i - 1], b'_' | b'.' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'))
    };
    ["rgb(", "rgba(", "white()", "black()"].into_iter().find(|call| called(call)).map(|_| "a colour call")
}

#[test]
fn no_story_names_a_colour() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let mut found = Vec::new();
    for file in files {
        for (n, line) in std::fs::read_to_string(&file).unwrap().lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if let Some(what) = is_colour(code) {
                found.push(format!("{}:{}: {what}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "colours written in code, not taken from the theme:\n{}", found.join("\n"));
}
