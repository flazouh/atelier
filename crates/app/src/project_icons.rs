//! Where a project's chosen icon is kept: a copy in the data folder, named from the project's place and the file's
//! extension, so the same choice writes the same file and another project's is never overwritten.
/// The copy's file name for the project at `place` and the chosen `file`: 16 hex digits of the place's hash, then the
/// extension in lower case (`png` when the file has none).
pub fn file_name(place: &str, file: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in place.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let extension = file.rsplit_once('.').map(|(_, e)| e.to_lowercase()).filter(|e| !e.is_empty() && e.len() <= 5).unwrap_or_else(|| "png".into());
    format!("{hash:016x}.{extension}")
}
#[cfg(test)]
mod tests {
    use super::file_name;
    #[test]
    fn the_same_choice_writes_the_same_file_and_another_project_another() {
        assert_eq!(file_name("~/code/x", "public/Logo.SVG"), file_name("~/code/x", "assets/mark.svg"));
        assert_ne!(file_name("~/code/x", "a.png"), file_name("~/code/y", "a.png"));
        assert!(file_name("~/code/x", "public/Logo.SVG").ends_with(".svg"));
        assert!(file_name("~/code/x", "noext").ends_with(".png"));
        assert_eq!(file_name("~/code/x", "a.png").len(), 16 + 1 + 3);
    }
}
