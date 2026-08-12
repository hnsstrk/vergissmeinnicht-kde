//! Einstellungen und Pfade. Persistenz als JSON unter
//! `~/.config/vergissmeinnicht/config.json` (XDG); die Replica liegt unter
//! `~/.local/share/vergissmeinnicht/replica/`, Backups daneben.
//!
//! Pendant zu `AppSettings`/`@AppStorage` der macOS-Version. Sync-Credentials
//! und der KI-API-Key liegen NICHT hier, sondern im Secret Service (KWallet)
//! — siehe `secrets.rs`.

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
    /// Eingeklappte Sidebar-Sektionen ("saved" | "projects" | "tags").
    pub collapsed_sections: Vec<String>,
    /// KI-Provider-Preset: "ollama" | "openrouter" | "custom".
    pub ai_provider: String,
    /// Basis-URL des OpenAI-kompatiblen Endpunkts (vom Preset vorbefüllt).
    pub ai_base_url: String,
    /// Modellname beim Provider; leer = KI nicht konfiguriert.
    /// Der API-Key ist bewusst KEIN Feld hier — er liegt im Secret Service.
    pub ai_model: String,
    /// Speech-to-Text-Backend: "openai-whisper" | "whisper-cpp".
    pub ai_stt_backend: String,
    /// Whisper-Modellname für das openai-whisper-Backend (z. B. "small").
    pub ai_whisper_model: String,
    /// Pfad zum whisper-cli-Binary (nur für das whisper-cpp-Backend).
    pub ai_whisper_cpp_binary: String,
    /// Pfad zur GGML-Modelldatei (nur für das whisper-cpp-Backend).
    pub ai_whisper_cpp_model: String,
    /// Kontextumfang der KI-Interpretation (AI-B1b, #31):
    /// "taxonomy" = nur Projekt-/Schlagwortnamen (Default, bisheriges
    /// Verhalten), "open_titles" = zusätzlich Titel offener Aufgaben,
    /// "all" = alle nicht gelöschten Aufgaben kompakt.
    pub ai_context_level: String,
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
            collapsed_sections: Vec::new(),
            ai_provider: "ollama".into(),
            ai_base_url: "http://localhost:11434/v1".into(),
            ai_model: String::new(),
            ai_stt_backend: "openai-whisper".into(),
            ai_whisper_model: "small".into(),
            ai_whisper_cpp_binary: String::new(),
            ai_whisper_cpp_model: String::new(),
            ai_context_level: "taxonomy".into(),
        }
    }
}

/// Basis-URL-Vorgabe je KI-Provider-Preset (Spec §4.3). „custom" (und alles
/// Unbekannte) liefert leer — dort bleibt die URL Handarbeit, die
/// Einstellungsseite lässt das Feld dann unangetastet.
pub fn ai_provider_default_url(provider: &str) -> &'static str {
    match provider {
        "ollama" => "http://localhost:11434/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        _ => "",
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
    fn sync_url_ueberlebt_speichern_und_echten_ladeweg() {
        // Akzeptanzkriterium #38: Konfiguration schreiben, über
        // `Settings::load()` (den echten Ladeweg über config_path) neu
        // einlesen — die Sync-Server-URL muss noch da sein, und die Felder
        // der übrigen Einstellungsseiten ebenso.
        let dir = std::env::temp_dir().join(format!("vmn-cfg-test-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let s = Settings {
            sync_server_url: "https://sync.example.org:8080".into(),
            ai_model: "test-modell".into(),
            language: "de".into(),
            ..Settings::default()
        };
        s.save().unwrap();
        let wieder = Settings::load();
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(wieder.sync_server_url, "https://sync.example.org:8080");
        assert_eq!(wieder.ai_model, "test-modell");
        assert_eq!(wieder.language, "de");
    }

    #[test]
    fn alle_felder_ueberleben_den_speicher_roundtrip() {
        // #38-Absicherung für alle Einstellungsseiten: Jede Seite speichert
        // die ganze Struktur — bleibt hier jedes Feld erhalten, kann kein
        // Speichervorgang einer Seite die Felder einer anderen verlieren.
        let s = Settings {
            default_filter: "today".into(),
            sort_key: "due".into(),
            sort_ascending: false,
            due_soon_days: 3,
            hide_completed: true,
            notify_overdue: true,
            auto_sync: "m15".into(),
            language: "en".into(),
            sync_server_url: "https://sync.example.org".into(),
            saved_searches: vec![SavedSearch {
                id: "s1".into(),
                name: "Suche".into(),
                query: "project:x".into(),
            }],
            last_overdue_count: 4,
            sidebar_width: 240,
            collapsed_sections: vec!["tags".into()],
            ai_provider: "custom".into(),
            ai_base_url: "https://llm.example/v1".into(),
            ai_model: "modell-x".into(),
            ai_stt_backend: "whisper-cpp".into(),
            ai_whisper_model: "medium".into(),
            ai_whisper_cpp_binary: "/usr/bin/whisper-cli".into(),
            ai_whisper_cpp_model: "/pfad/ggml.bin".into(),
            ai_context_level: "all".into(),
        };
        let raw = serde_json::to_string(&s).unwrap();
        let wieder: Settings = serde_json::from_str(&raw).unwrap();
        let nochmal = serde_json::to_string(&wieder).unwrap();
        assert_eq!(raw, nochmal);
        assert_eq!(wieder.sync_server_url, "https://sync.example.org");
    }

    #[test]
    fn sync_url_bleibt_beim_laden_alter_configs() {
        // Fehlende Schlüssel (ältere Version) dürfen die vorhandene URL
        // nicht verdrängen — serde-Default greift nur für fehlende Felder.
        let wieder: Settings =
            serde_json::from_str(r#"{"sync_server_url":"https://s.example"}"#).unwrap();
        assert_eq!(wieder.sync_server_url, "https://s.example");
        assert_eq!(wieder.default_filter, "inbox");
    }

    #[test]
    fn sidebar_settings_roundtrip() {
        let mut s = Settings::default();
        assert_eq!(s.sidebar_width, 0);
        s.sidebar_width = 247;
        s.collapsed_sections = vec!["tags".into()];
        let raw = serde_json::to_string(&s).unwrap();
        let wieder: Settings = serde_json::from_str(&raw).unwrap();
        assert_eq!(wieder.sidebar_width, 247);
        assert_eq!(wieder.collapsed_sections, vec!["tags".to_string()]);
    }

    #[test]
    fn old_config_without_sidebar_width_defaults_to_zero() {
        // Config aus einer Version vor 0.2.3 — Felder fehlen, Defaults greifen.
        let wieder: Settings = serde_json::from_str(r#"{"default_filter":"todo"}"#).unwrap();
        assert_eq!(wieder.sidebar_width, 0);
        assert!(wieder.collapsed_sections.is_empty());
        assert_eq!(wieder.default_filter, "todo");
    }

    #[test]
    fn ai_defaults() {
        let s = Settings::default();
        assert_eq!(s.ai_provider, "ollama");
        assert_eq!(s.ai_base_url, "http://localhost:11434/v1");
        assert!(s.ai_model.is_empty());
        assert_eq!(s.ai_stt_backend, "openai-whisper");
        assert_eq!(s.ai_whisper_model, "small");
        assert!(s.ai_whisper_cpp_binary.is_empty());
        assert!(s.ai_whisper_cpp_model.is_empty());
        assert_eq!(s.ai_context_level, "taxonomy");
    }

    #[test]
    fn provider_presets_liefern_basis_urls() {
        // Presets aus Spec §4.3: Ollama lokal, OpenRouter-Cloud; „custom"
        // (und Unbekanntes) bleibt leer und überschreibt nichts.
        assert_eq!(ai_provider_default_url("ollama"), "http://localhost:11434/v1");
        assert_eq!(ai_provider_default_url("openrouter"), "https://openrouter.ai/api/v1");
        assert_eq!(ai_provider_default_url("custom"), "");
        assert_eq!(ai_provider_default_url("unbekannt"), "");
    }

    #[test]
    fn api_key_never_lands_in_config_json() {
        // Der KI-API-Key liegt im Secret Service (secrets.rs) — die
        // serialisierte Config darf kein Key-Feld enthalten.
        let raw = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!raw.to_lowercase().contains("api_key"));
        assert!(!raw.to_lowercase().contains("apikey"));
    }

    #[test]
    fn old_config_without_ai_fields_gets_ai_defaults() {
        // Config aus einer Version vor der KI-Integration — Defaults greifen.
        let wieder: Settings = serde_json::from_str(r#"{"default_filter":"todo"}"#).unwrap();
        assert_eq!(wieder.ai_provider, "ollama");
        assert_eq!(wieder.ai_base_url, "http://localhost:11434/v1");
        assert!(wieder.ai_model.is_empty());
        assert_eq!(wieder.ai_stt_backend, "openai-whisper");
        assert_eq!(wieder.ai_whisper_model, "small");
        // AI-B1b: Bestandsinstallationen behalten das bisherige Verhalten.
        assert_eq!(wieder.ai_context_level, "taxonomy");
        assert_eq!(wieder.default_filter, "todo");
    }
}
