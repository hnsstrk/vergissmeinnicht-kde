//! Einstellungen und Pfade. Persistenz als JSON unter
//! `~/.config/vergissmeinnicht/config.json` (XDG); die Replica liegt unter
//! `~/.local/share/vergissmeinnicht/replica/`, Backups daneben.
//!
//! Pendant zu `AppSettings`/`@AppStorage` der macOS-Version. Sync-Credentials
//! liegen NICHT hier, sondern im Secret Service (KWallet) — siehe `secrets.rs`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Standard-Sidebar-Filter beim Start (Filter-Key, z.B. "inbox").
    pub default_filter: String,
    /// Standard-Sortierung ("id" | "description" | "entry" | "due" | "project").
    pub sort_key: String,
    pub sort_ascending: bool,
    /// Fenster für „Bald fällig" in Tagen.
    pub due_soon_days: i64,
    pub hide_completed: bool,
    /// Zusammenfassungs-Benachrichtigung überfälliger Aufgaben beim Start (opt-in).
    pub notify_overdue: bool,
    /// Auto-Sync-Modus: "manual" | "m5" | "m15" | "m60" | "immediate".
    pub auto_sync: String,
    /// UI-Sprache: "" = Systemsprache, sonst z. B. "de" oder "en".
    /// Wird beim Start angewendet (Neustart nötig, wie in der macOS-Version).
    pub language: String,
    /// Sync-Server-URL. Nicht geheim — Client-ID und Secret liegen im Secret Service.
    pub sync_server_url: String,
    pub saved_searches: Vec<SavedSearch>,
    /// Letzter gemeldeter Überfällig-Zähler (Anti-Spam für die Start-Notification).
    pub last_overdue_count: i64,
    /// Sidebar-Breite in Pixeln; 0 = Standardbreite des Themes.
    pub sidebar_width: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_filter: "inbox".into(),
            sort_key: "id".into(),
            sort_ascending: true,
            due_soon_days: 7,
            hide_completed: false,
            notify_overdue: false,
            auto_sync: "manual".into(),
            language: String::new(),
            sync_server_url: String::new(),
            saved_searches: Vec::new(),
            last_overdue_count: 0,
            sidebar_width: 0,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vergissmeinnicht")
        .join("config.json")
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vergissmeinnicht")
}

pub fn replica_dir() -> PathBuf {
    data_dir().join("replica")
}

pub fn backup_dir() -> PathBuf {
    data_dir().join("backups")
}

impl Settings {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_width_roundtrip() {
        let mut s = Settings::default();
        assert_eq!(s.sidebar_width, 0);
        s.sidebar_width = 247;
        let raw = serde_json::to_string(&s).unwrap();
        let wieder: Settings = serde_json::from_str(&raw).unwrap();
        assert_eq!(wieder.sidebar_width, 247);
    }

    #[test]
    fn old_config_without_sidebar_width_defaults_to_zero() {
        // Config aus einer Version vor 0.2.3 — Feld fehlt, Default muss greifen.
        let wieder: Settings = serde_json::from_str(r#"{"default_filter":"todo"}"#).unwrap();
        assert_eq!(wieder.sidebar_width, 0);
        assert_eq!(wieder.default_filter, "todo");
    }
}
