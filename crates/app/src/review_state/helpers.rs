/// Where the record of session `id` lives in the project's data folder.
pub fn record_path(id: &str) -> String {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("review/{safe}.json")
}
