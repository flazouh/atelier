pub(super) fn trim_slash(path: &str) -> &str {
    if path.len() > 1 { path.trim_end_matches('/') } else { path }
}

/// FNV-1a, 64 bits: the same on every machine and every run, unlike the standard library's hasher.
pub(super) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

/// The prefix of short ids from a project's name: its first three letters or digits, in capitals.
/// "atelier" gives "ATE", "api-server" gives "API", and a name with none gives "TSK".
pub fn prefix_for(name: &str) -> String {
    let prefix: String = name.chars().filter(char::is_ascii_alphanumeric).take(3).collect::<String>().to_uppercase();
    if prefix.is_empty() { "TSK".to_string() } else { prefix }
}
