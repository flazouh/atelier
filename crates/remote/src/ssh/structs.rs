/// The host's system and architecture, as atelier names builds: `linux-x86_64`, `darwin-aarch64`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    pub system: String,
    pub arch: String,
}

impl Platform {
    /// From `uname -sm`: `Linux x86_64`, `Darwin arm64`.
    pub fn from_uname(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        let system = parts.next()?.to_lowercase();
        let arch = match parts.next()? {
            "arm64" => "aarch64".to_string(),
            "amd64" => "x86_64".to_string(),
            other => other.to_string(),
        };
        Some(Self { system, arch })
    }

    /// This machine's.
    pub fn here() -> Self {
        let system = match std::env::consts::OS {
            "macos" => "darwin",
            other => other,
        };
        Self { system: system.to_string(), arch: std::env::consts::ARCH.to_string() }
    }

    pub fn name(&self) -> String {
        format!("{}-{}", self.system, self.arch)
    }

    /// For a person: "Linux x86_64", "macOS aarch64".
    pub fn describe(&self) -> String {
        let system = match self.system.as_str() {
            "linux" => "Linux".to_string(),
            "darwin" => "macOS".to_string(),
            other => other.to_string(),
        };
        format!("{system} {}", self.arch)
    }
}
