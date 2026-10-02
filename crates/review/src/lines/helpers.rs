/// Rows joined back into a text with its final line end as it was.
pub(crate) fn join(rows: impl IntoIterator<Item = impl AsRef<str>>, final_newline: bool) -> String {
    let mut out = String::new();
    let mut any = false;
    for row in rows {
        if any {
            out.push('\n');
        }
        out.push_str(row.as_ref());
        any = true;
    }
    if any && final_newline {
        out.push('\n');
    }
    out
}
