//! Sync-Credentials im Secret Service (unter KDE: KWallet 6 stellt die
//! org.freedesktop.secrets-API bereit). Pendant zum macOS-Keychain-Store.
//!
//! Gespeichert werden Client-ID und Encryption-Secret; die Server-URL ist nicht
//! geheim und liegt in der Config (analog macOS: URL in UserDefaults wäre ok,
//! Secrets nie).

use keyring::Entry;

const SERVICE: &str = "de.hnsstrk.vergissmeinnicht.sync";

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, key).map_err(|e| e.to_string())
}

pub fn get(key: &str) -> Result<Option<String>, String> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Leerer Wert löscht den Eintrag (idempotent).
pub fn set(key: &str, value: &str) -> Result<(), String> {
    let e = entry(key)?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

pub const KEY_CLIENT_ID: &str = "client-id";
pub const KEY_SECRET: &str = "encryption-secret";
