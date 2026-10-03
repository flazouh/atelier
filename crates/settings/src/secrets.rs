//! Secrets atelier keeps for the reader, such as an API key: in the system's keychain (the macOS Keychain, the
//! Secret Service on Linux), never in the settings file.
use std::{collections::HashMap, io, sync::Mutex};

/// The keychain entry that holds the OpenRouter API key.
pub const OPENROUTER_KEY: &str = "openrouter-api-key";

/// The service every entry of atelier's is filed under.
const SERVICE: &str = "atelier";

pub trait Secrets: Send + Sync {
    /// `None` when nothing is kept under `name`.
    fn read(&self, name: &str) -> io::Result<Option<String>>;
    fn write(&self, name: &str, secret: &str) -> io::Result<()>;
    /// Forgetting what is not kept is not an error.
    fn forget(&self, name: &str) -> io::Result<()>;
}

/// The system's keychain.
pub struct Keychain;

impl Keychain {
    fn entry(name: &str) -> io::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, name).map_err(io::Error::other)
    }
}

impl Secrets for Keychain {
    fn read(&self, name: &str) -> io::Result<Option<String>> {
        match Self::entry(name)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(io::Error::other(error)),
        }
    }

    fn write(&self, name: &str, secret: &str) -> io::Result<()> {
        Self::entry(name)?.set_password(secret).map_err(io::Error::other)
    }

    fn forget(&self, name: &str) -> io::Result<()> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(io::Error::other(error)),
        }
    }
}

/// Secrets that last as long as the value, for tests and the gallery.
#[derive(Default)]
pub struct InMemory(Mutex<HashMap<String, String>>);

impl Secrets for InMemory {
    fn read(&self, name: &str) -> io::Result<Option<String>> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).get(name).cloned())
    }

    fn write(&self, name: &str, secret: &str) -> io::Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(name.into(), secret.into());
        Ok(())
    }

    fn forget(&self, name: &str) -> io::Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
