//! Sync-Credentials und KI-API-Key im Secret Service (unter KDE: KWallet 6
//! stellt die org.freedesktop.secrets-API bereit). Pendant zum
//! macOS-Keychain-Store.
//!
//! Gespeichert werden Client-ID, Encryption-Secret und der KI-API-Key; die
//! Server-URLs sind nicht geheim und liegen in der Config (analog macOS: URL
//! in UserDefaults wäre ok, Secrets nie).

use keyring::Entry;

/// Sync-Credentials (Client-ID, Encryption-Secret).
const SERVICE_SYNC: &str = "de.hnsstrk.vergissmeinnicht.sync";
/// KI-API-Key — eigener Service-String, kein Untermieter im Sync-Service.
const SERVICE_AI: &str = "de.hnsstrk.vergissmeinnicht.ai";

fn entry(service: &str, key: &str) -> Result<Entry, String> {
    Entry::new(service, key).map_err(|e| e.to_string())
}

fn get_in(service: &str, key: &str) -> Result<Option<String>, String> {
    match entry(service, key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Leerer Wert löscht den Eintrag (idempotent).
fn set_in(service: &str, key: &str, value: &str) -> Result<(), String> {
    let e = entry(service, key)?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

/// Sync-Credential lesen (Service `…vergissmeinnicht.sync`).
pub fn get(key: &str) -> Result<Option<String>, String> {
    get_in(SERVICE_SYNC, key)
}

/// Sync-Credential schreiben; leerer Wert löscht den Eintrag (idempotent).
pub fn set(key: &str, value: &str) -> Result<(), String> {
    set_in(SERVICE_SYNC, key, value)
}

pub const KEY_CLIENT_ID: &str = "client-id";
pub const KEY_SECRET: &str = "encryption-secret";

const KEY_AI_API_KEY: &str = "api-key";

/// KI-API-Key lesen (Service `…vergissmeinnicht.ai`).
pub fn get_ai_api_key() -> Result<Option<String>, String> {
    get_in(SERVICE_AI, KEY_AI_API_KEY)
}

/// KI-API-Key schreiben; leerer Wert löscht den Eintrag (idempotent).
/// Genutzt ab Story AI-A3.
#[allow(dead_code)]
pub fn set_ai_api_key(value: &str) -> Result<(), String> {
    set_in(SERVICE_AI, KEY_AI_API_KEY, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Echter Secret-Service-Roundtrip — braucht eine entsperrte Session mit
    /// laufendem org.freedesktop.secrets-Dienst, daher `#[ignore]`:
    ///
    ///     cargo test -p vergissmeinnicht-app -- --ignored secrets
    #[test]
    #[ignore]
    fn roundtrip_against_live_secret_service() {
        let key = "test-roundtrip";
        set(key, "geheimer-testwert").expect("set");
        assert_eq!(get(key).expect("get").as_deref(), Some("geheimer-testwert"));
        // Leerer Wert löscht (idempotent).
        set(key, "").expect("delete");
        set(key, "").expect("delete idempotent");
        assert_eq!(get(key).expect("get nach delete"), None);
    }

    /// Wie oben, für den KI-API-Key im eigenen Service — braucht ebenfalls
    /// eine entsperrte Session, daher `#[ignore]`.
    #[test]
    #[ignore]
    fn ai_key_roundtrip_against_live_secret_service() {
        set_ai_api_key("geheimer-ki-testwert").expect("set");
        assert_eq!(
            get_ai_api_key().expect("get").as_deref(),
            Some("geheimer-ki-testwert")
        );
        // Leerer Wert löscht (idempotent).
        set_ai_api_key("").expect("delete");
        set_ai_api_key("").expect("delete idempotent");
        assert_eq!(get_ai_api_key().expect("get nach delete"), None);
    }
}
