//! macOS Keychain-backed storage for API keys and other secrets.
//!
//! Secrets are never written to SQLite. The UI can set and clear them, and
//! check whether a value is present, but the plaintext is only read back
//! internally when a backend needs it (e.g. injecting `OPENAI_API_KEY`).

use keyring::Entry;

const SERVICE: &str = "com.liamgk.buddy";

/// Keys that the Settings UI is allowed to manage.
pub const KNOWN_SECRETS: &[&str] = &[
    "openai_api_key",
    "cursor_api_key",
    "smtp_password",
    "calcom_api_key",
    "calcom_webhook_secret",
];

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
