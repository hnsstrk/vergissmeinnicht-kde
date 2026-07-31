//! KI-Modul (Spec §4.1) — Qt-frei und ohne Bridge-Abhängigkeiten, damit
//! alles hier ohne Qt-Laufzeit testbar ist (wie `parsers.rs`). Die Bridge
//! ruft dieses Modul erst ab Story AI-A3 auf.

pub mod client;
