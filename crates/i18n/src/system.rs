use super::Locale;

/// The language the reader's system is set to, or English. `ATELIER_LANG` overrides it, for trying a language.
///
/// On Linux that is `LC_ALL`, `LC_MESSAGES`, `LANGUAGE` (a list) and `LANG`, in the order the C library reads them. A Mac app
/// started from the Dock has none of those, so there it is the list in System Settings > Language & Region.
pub fn system_locale() -> Locale {
    let tags = from_env(|name| std::env::var(name).ok());
    let tags = if cfg!(target_os = "macos") && std::env::var("ATELIER_LANG").is_err() { mac_languages().unwrap_or(tags) } else { tags };
    Locale::resolve(tags.iter().map(String::as_str))
}

/// The tags the environment names, most wanted first.
pub(crate) fn from_env(get: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(forced) = get("ATELIER_LANG").filter(|v| !v.is_empty()) {
        return vec![forced];
    }
    for name in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        let Some(value) = get(name).filter(|v| !v.is_empty()) else { continue };
        tags.extend(value.split(':').filter(|t| !t.is_empty()).map(str::to_string));
        // `LC_ALL` set to a language settles it: the ones after it are not read.
        if name == "LC_ALL" {
            break;
        }
    }
    tags
}

/// `defaults read -g AppleLanguages` prints a list: `(\n    "fr-FR",\n    "en-US"\n)`.
pub(crate) fn parse_apple_languages(output: &str) -> Vec<String> {
    output.lines().map(|l| l.trim().trim_end_matches(',').trim_matches('"')).filter(|l| !l.is_empty() && *l != "(" && *l != ")").map(str::to_string).collect()
}

fn mac_languages() -> Option<Vec<String>> {
    let out = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
    let tags = parse_apple_languages(&String::from_utf8_lossy(&out.stdout));
    (!tags.is_empty()).then_some(tags)
}
