//! macOS Keychain-backed storage for API keys and other secrets.
//!
//! Secrets are never written to SQLite. The UI can set and clear them, and
//! check whether a value is present, but the plaintext is only read back
//! internally when a backend needs it (e.g. injecting `OPENAI_API_KEY`).

use keyring::Entry;

const SERVICE: &str = "com.liamgk.buddy";

/// Secret keys owned by the app shell (code agent backends) rather than any
/// single plugin.
const SHELL_SECRETS: &[&str] = &["openai_api_key", "cursor_api_key"];

/// Keys that the Settings UI is allowed to manage: shell-owned keys plus
/// whatever each `BuddyPlugin::secret_keys()` declares, so a new plugin's
/// secrets don't need a second edit here.
pub fn known_secrets() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = SHELL_SECRETS.to_vec();
    for plugin in buddy_plugins::all_builtin_plugins() {
        keys.extend(plugin.secret_keys());
    }
    keys
}

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, key).map_err(|e| e.to_string())
}

pub fn set_secret(key: &str, value: &str) -> Result<(), String> {
    entry(key)?.set_password(value).map_err(|e| e.to_string())
}

pub fn get_secret(key: &str) -> Result<Option<String>, String> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_secret(key: &str) -> Result<(), String> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn has_secret(key: &str) -> bool {
    matches!(get_secret(key), Ok(Some(_)))
}

pub fn is_known_secret(key: &str) -> bool {
    known_secrets().contains(&key)
}
