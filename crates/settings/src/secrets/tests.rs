use super::{InMemory, Keychain, Secrets};

const KEY_NAME: &str = "test-key";
const FIRST_KEY: &str = "sk-first";
const SECOND_KEY: &str = "sk-second";

/// What every keeper of secrets must do.
fn keeps_replaces_and_forgets(secrets: &dyn Secrets, name: &str) {
    assert_eq!(secrets.read(name).unwrap(), None, "nothing kept yet");

    secrets.write(name, FIRST_KEY).unwrap();
    assert_eq!(secrets.read(name).unwrap().as_deref(), Some(FIRST_KEY));

    secrets.write(name, SECOND_KEY).unwrap();
    assert_eq!(secrets.read(name).unwrap().as_deref(), Some(SECOND_KEY), "a new key replaces the old");

    secrets.forget(name).unwrap();
    assert_eq!(secrets.read(name).unwrap(), None);
    secrets.forget(name).unwrap();
}

#[test]
fn memory_keeps_replaces_and_forgets() {
    keeps_replaces_and_forgets(&InMemory::default(), KEY_NAME);
}

/// Writes to this machine's keychain, so it runs only when asked: `cargo test -p atelier-settings -- --ignored`.
#[test]
#[ignore]
fn the_system_keychain_keeps_replaces_and_forgets() {
    keeps_replaces_and_forgets(&Keychain, &format!("{KEY_NAME}-{}", std::process::id()));
}
