fn rust_line(i: usize) -> [String; 6] {
    [
        format!("/// Adds {i}, and says so."),
        format!("pub fn add_{i}(value: u64) -> Result<u64, String> {{"),
        format!("    let label = \"step {i}\"; /* a comment */"),
        format!("    value.checked_add({i}).ok_or(format!(\"{{label}} overflowed\"))"),
        "}".to_string(),
        String::new(),
    ]
}

pub(super) fn rust_file(lines: usize) -> String {
    (0..).flat_map(rust_line).take(lines).collect::<Vec<_>>().join("\n")
}

pub(super) fn diff(rows: usize) -> String {
    let body: Vec<String> = rust_file(rows)
        .lines()
        .enumerate()
        .map(|(i, line)| match i % 12 {
            3 => format!("-{line}"),
            4 => format!("+{line} // changed"),
            _ => format!(" {line}"),
        })
        .collect();
    format!("@@ -1,{rows} +1,{rows} @@\n{}", body.join("\n"))
}
