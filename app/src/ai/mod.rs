//! KI-Modul (Spec §4.1) — Qt-frei und ohne Bridge-Abhängigkeiten, damit
//! alles hier ohne Qt-Laufzeit testbar ist (wie `parsers.rs`). Die Bridge
//! ruft dieses Modul erst ab Story AI-A3 auf.

pub mod client;
pub mod mock;

use client::{AiError, Llm};

/// Fabrik-Weiche (Spec §8): Bei gesetztem `VMN_AI_MOCK` liefert sie den
/// Konserven-Mock, sonst den echten HTTP-Client aus den Einstellungen.
/// Die eine Stelle, an der die Bridge ab Story AI-A3 andockt. `Send + Sync`,
/// weil ab AI-A3 mehrere Worker-Threads gleichzeitig Anfragen halten.
/// Ein fehlerhaft konfigurierter Mock ist ein Fehler, kein stiller Rückfall
/// auf den echten Client (Konserven-Format: siehe Modulkommentar in `mock`).
pub fn make_llm(
    settings: &crate::config::Settings,
) -> Result<Box<dyn Llm + Send + Sync>, AiError> {
    match mock::CannedLlm::from_env()? {
        Some(canned) => Ok(Box::new(canned)),
        None => Ok(Box::new(client::LlmClient::from_settings(settings)?)),
    }
}
