//! Konserven-Mock für den [`Llm`]-Trait (Story AI-B3a, Spec §8) — produktiv
//! kompiliert, damit ein echter App-Lauf per Umgebungsvariable ohne Netz und
//! ohne Modell auskommt:
//!
//! * `VMN_AI_MOCK=<pfad>` — JSON-Datei mit den Konservenantworten. Format:
//!   ein Array von Einträgen; je Eintrag `content` (JSON-Objekt = fertige
//!   Modellantwort, oder String = roher Antworttext, etwa um kaputte
//!   Modellausgaben zu simulieren) und optional `delay_ms`:
//!
//!   ```json
//!   [
//!     {"content": {"title": "Milch kaufen", "due": "morgen"}},
//!     {"content": "kein json", "delay_ms": 250}
//!   ]
//!   ```
//!
//!   Die Antworten werden in Aufruf-Reihenfolge serviert; nach der letzten
//!   wiederholt sich die letzte — ein App-Lauf darf nie leerlaufen.
//! * `VMN_AI_MOCK_DELAY_MS=<zahl>` — Grundlatenz je Antwort in Millisekunden
//!   (Standard 0); `delay_ms` am Eintrag geht vor. Die steuerbare Latenz ist
//!   die Voraussetzung, um ab AI-A3 den Generationszähler (Verwerfen
//!   veralteter Ergebnisse) zu beobachten: zwei laufende Anfragen können
//!   sich überholen.
//!
//! Eine fehlerhaft benannte oder unlesbare Konserven-Quelle ist ein
//! [`AiError::Config`] — kein stiller Rückfall auf den echten Client.

use super::client::{AiError, ChatMessage, Llm};
use serde::Deserialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// Umgebungsvariable: Pfad zur Konserven-Datei; leer = Mock aus.
pub const ENV_MOCK: &str = "VMN_AI_MOCK";

/// Umgebungsvariable: Grundlatenz je Antwort in Millisekunden.
pub const ENV_MOCK_DELAY: &str = "VMN_AI_MOCK_DELAY_MS";

/// Roh-Eintrag der Konserven-Datei. `deny_unknown_fields`, damit Tippfehler
/// (z. B. `dely_ms`) laut scheitern statt still die Grundlatenz zu nutzen.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Eintrag {
    content: serde_json::Value,
    delay_ms: Option<u64>,
}

/// Eine geladene Konservenantwort: fertiger Antworttext plus optionale
/// Latenz, die die Grundlatenz übersteuert.
#[derive(Clone)]
struct Antwort {
    text: String,
    verzoegerung: Option<Duration>,
}

/// Produktiver Mock des [`Llm`]-Traits: serviert Konservenantworten in
/// Aufruf-Reihenfolge. `Mutex` statt `RefCell`, weil ab AI-A3 mehrere
/// Worker-Threads gleichzeitig Anfragen halten.
pub struct CannedLlm {
    antworten: Vec<Antwort>,
    position: Mutex<usize>,
    basis_verzoegerung: Duration,
}

impl CannedLlm {
    /// Liest die beiden Umgebungsvariablen: `Ok(None)` wenn `VMN_AI_MOCK`
    /// fehlt oder leer ist (Mock aus), sonst der geladene Mock oder ein
    /// Konfigurationsfehler.
    pub fn from_env() -> Result<Option<Self>, AiError> {
        let pfad = match std::env::var(ENV_MOCK) {
            Ok(p) if !p.is_empty() => p,
            _ => return Ok(None),
        };
        let basis = match std::env::var(ENV_MOCK_DELAY) {
            Ok(wert) if !wert.is_empty() => {
                let ms: u64 = wert
                    .parse()
                    .map_err(|_| AiError::Config(format!("{ENV_MOCK_DELAY}: keine Zahl: {wert}")))?;
                Duration::from_millis(ms)
            }
            _ => Duration::ZERO,
        };
        Self::from_datei(Path::new(&pfad), basis).map(Some)
    }

    /// Lädt die Konserven-Datei (Format siehe Modulkommentar).
    pub fn from_datei(pfad: &Path, basis_verzoegerung: Duration) -> Result<Self, AiError> {
        let text = std::fs::read_to_string(pfad).map_err(|e| {
            AiError::Config(format!("Mock-Konserven {} nicht lesbar: {e}", pfad.display()))
        })?;
        let eintraege: Vec<Eintrag> = serde_json::from_str(&text)
            .map_err(|e| AiError::Config(format!("Mock-Konserven {}: {e}", pfad.display())))?;
        if eintraege.is_empty() {
            return Err(AiError::Config(format!(
                "Mock-Konserven {}: leeres Array — mindestens eine Antwort nötig",
                pfad.display()
            )));
        }
        let antworten = eintraege
            .into_iter()
            .map(|e| Antwort {
                // String wörtlich übernehmen (roher Antworttext), alles
                // andere als JSON-Text servieren.
                text: match e.content {
                    serde_json::Value::String(s) => s,
                    wert => wert.to_string(),
                },
                verzoegerung: e.delay_ms.map(Duration::from_millis),
            })
            .collect();
        Ok(Self { antworten, position: Mutex::new(0), basis_verzoegerung })
    }
}

impl Llm for CannedLlm {
    fn chat(&self, _messages: &[ChatMessage]) -> Result<String, AiError> {
        // Antwort unter dem Lock ziehen, aber außerhalb schlafen — sonst
        // serialisiert der Mutex die Anfragen und nichts kann sich überholen.
        let antwort = {
            let mut position = self.position.lock().expect("Mock-Position");
            let index = (*position).min(self.antworten.len() - 1);
            *position = position.saturating_add(1);
            self.antworten[index].clone()
        };
        let verzoegerung = antwort.verzoegerung.unwrap_or(self.basis_verzoegerung);
        if !verzoegerung.is_zero() {
            std::thread::sleep(verzoegerung);
        }
        Ok(antwort.text)
    }

    fn list_models(&self) -> Result<Vec<String>, AiError> {
        Ok(vec!["vmn-mock".into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{mpsc, Arc};
    use std::time::Instant;

    /// Die Prozessumgebung ist global — alle Tests, die `VMN_AI_MOCK*`
    /// anfassen, laufen über [`mit_env`] und damit unter diesem Lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Führt `test` mit exakt den angegebenen Mock-Variablen aus: alle
    /// anderen sind entfernt, danach wird aufgeräumt — auch bei Panik.
    fn mit_env(paare: &[(&str, &str)], test: impl FnOnce()) {
        let _sperre = ENV_LOCK.lock().unwrap_or_else(|vergiftet| vergiftet.into_inner());
        std::env::remove_var(ENV_MOCK);
        std::env::remove_var(ENV_MOCK_DELAY);
        for (name, wert) in paare {
            std::env::set_var(name, wert);
        }
        let ergebnis = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
        std::env::remove_var(ENV_MOCK);
        std::env::remove_var(ENV_MOCK_DELAY);
        if let Err(panik) = ergebnis {
            std::panic::resume_unwind(panik);
        }
    }

    fn schreibe_konserven(inhalt: &str) -> tempfile::NamedTempFile {
        let mut datei = tempfile::NamedTempFile::new().expect("Tempdatei");
        datei.write_all(inhalt.as_bytes()).expect("Konserven schreiben");
        datei
    }

    fn antwort(text: &str, verzoegerung: Option<Duration>) -> Antwort {
        Antwort { text: text.into(), verzoegerung }
    }

    fn mock(antworten: Vec<Antwort>, basis: Duration) -> CannedLlm {
        CannedLlm { antworten, position: Mutex::new(0), basis_verzoegerung: basis }
    }

    // ─── Konserven-Laden ────────────────────────────────────────────────────

    #[test]
    fn laden_objekt_und_string_content() {
        let datei = schreibe_konserven(
            r#"[
                {"content": {"title": "Milch kaufen", "due": "morgen"}},
                {"content": "kein json", "delay_ms": 250}
            ]"#,
        );
        let mock = CannedLlm::from_datei(datei.path(), Duration::ZERO).unwrap();
        assert_eq!(mock.antworten.len(), 2);
        // Objekt-Content wird als JSON-Text serviert (Schlüsselreihenfolge
        // ist serde-Sache — über Parse-Rückweg vergleichen).
        let erste: serde_json::Value = serde_json::from_str(&mock.antworten[0].text).unwrap();
        assert_eq!(erste, serde_json::json!({"title": "Milch kaufen", "due": "morgen"}));
        assert_eq!(mock.antworten[0].verzoegerung, None);
        // String-Content bleibt wörtlich (simuliert kaputte Modellausgabe).
        assert_eq!(mock.antworten[1].text, "kein json");
        assert_eq!(mock.antworten[1].verzoegerung, Some(Duration::from_millis(250)));
    }

    #[test]
    fn laden_fehler_datei_fehlt() {
        assert!(matches!(
            CannedLlm::from_datei(Path::new("/gibt/es/nicht.json"), Duration::ZERO),
            Err(AiError::Config(_))
        ));
    }

    #[test]
    fn laden_fehler_kein_array_oder_leer() {
        let objekt = schreibe_konserven(r#"{"content": "x"}"#);
        assert!(matches!(
            CannedLlm::from_datei(objekt.path(), Duration::ZERO),
            Err(AiError::Config(_))
        ));
        let leer = schreibe_konserven("[]");
        assert!(matches!(
            CannedLlm::from_datei(leer.path(), Duration::ZERO),
            Err(AiError::Config(_))
        ));
    }

    #[test]
    fn laden_fehler_unbekanntes_feld() {
        // Tippfehler im Eintrag scheitert laut statt still ohne Latenz.
        let datei = schreibe_konserven(r#"[{"content": "x", "dely_ms": 3}]"#);
        assert!(matches!(
            CannedLlm::from_datei(datei.path(), Duration::ZERO),
            Err(AiError::Config(_))
        ));
    }

    // ─── Servier-Reihenfolge ────────────────────────────────────────────────

    #[test]
    fn chat_serviert_in_reihenfolge_letzte_wiederholt_sich() {
        let mock = mock(
            vec![antwort("erste", None), antwort("zweite", None)],
            Duration::ZERO,
        );
        assert_eq!(mock.chat(&[]).unwrap(), "erste");
        assert_eq!(mock.chat(&[]).unwrap(), "zweite");
        // Nach der letzten Antwort wiederholt sich die letzte.
        assert_eq!(mock.chat(&[]).unwrap(), "zweite");
        assert_eq!(mock.chat(&[]).unwrap(), "zweite");
    }

    #[test]
    fn list_models_liefert_mock_modell() {
        let mock = mock(vec![antwort("x", None)], Duration::ZERO);
        assert_eq!(mock.list_models().unwrap(), vec!["vmn-mock".to_string()]);
    }

    // ─── Latenz ─────────────────────────────────────────────────────────────

    #[test]
    fn grundlatenz_wirkt() {
        let mock = mock(vec![antwort("x", None)], Duration::from_millis(120));
        let start = Instant::now();
        mock.chat(&[]).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(120));
    }

    #[test]
    fn eintrags_latenz_laesst_zwei_anfragen_ueberholen() {
        // Kern von AI-B3a: zwei gleichzeitig laufende Anfragen überholen
        // sich. Egal welcher Thread welche Konserve zieht — die langsame
        // (400 ms) kommt nach der schnellen (10 ms) an. Grundlatenz bewusst
        // riesig (5 s): käme sie zum Zug, risse der Test die Schranke unten.
        let mock = Arc::new(mock(
            vec![
                antwort("langsam", Some(Duration::from_millis(400))),
                antwort("schnell", Some(Duration::from_millis(10))),
            ],
            Duration::from_secs(5),
        ));
        let start = Instant::now();
        let (sender, empfaenger) = mpsc::channel();
        for _ in 0..2 {
            let mock = Arc::clone(&mock);
            let sender = sender.clone();
            std::thread::spawn(move || {
                let text = mock.chat(&[]).unwrap();
                sender.send(text).expect("Ergebnis melden");
            });
        }
        assert_eq!(empfaenger.recv().unwrap(), "schnell");
        assert_eq!(empfaenger.recv().unwrap(), "langsam");
        // Eintrags-Latenz hat die 5-s-Grundlatenz übersteuert.
        assert!(start.elapsed() < Duration::from_secs(4));
    }

    // ─── Env-Weiche ─────────────────────────────────────────────────────────

    #[test]
    fn from_env_ohne_variable_ist_none() {
        mit_env(&[], || {
            assert!(CannedLlm::from_env().unwrap().is_none());
        });
        // Leerer Wert zählt wie „nicht gesetzt".
        mit_env(&[(ENV_MOCK, "")], || {
            assert!(CannedLlm::from_env().unwrap().is_none());
        });
    }

    #[test]
    fn from_env_laedt_konserven_und_latenz() {
        let datei = schreibe_konserven(r#"[{"content": "aus env"}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad), (ENV_MOCK_DELAY, "25")], || {
            let mock = CannedLlm::from_env().unwrap().expect("Mock aktiv");
            assert_eq!(mock.basis_verzoegerung, Duration::from_millis(25));
            assert_eq!(mock.chat(&[]).unwrap(), "aus env");
        });
    }

    #[test]
    fn from_env_fehler_bei_kaputter_latenz_oder_datei() {
        let datei = schreibe_konserven(r#"[{"content": "x"}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad), (ENV_MOCK_DELAY, "abc")], || {
            assert!(matches!(CannedLlm::from_env(), Err(AiError::Config(_))));
        });
        mit_env(&[(ENV_MOCK, "/gibt/es/nicht.json")], || {
            assert!(matches!(CannedLlm::from_env(), Err(AiError::Config(_))));
        });
    }

    #[test]
    fn fabrik_liefert_mock_bei_gesetzter_env_var() {
        // Volle Weiche über `make_llm`: Konserve kommt durch den Trait an,
        // inklusive `complete_json`-Pfad (Objekt-Konserve → Objekt).
        let datei = schreibe_konserven(r#"[{"content": {"title": "Milch kaufen"}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad)], || {
            let llm = crate::ai::make_llm(&crate::config::Settings::default()).unwrap();
            let wert = llm
                .complete_json(&[ChatMessage::user("Milch kaufen")])
                .unwrap();
            assert_eq!(wert["title"], "Milch kaufen");
        });
    }

    #[test]
    fn fabrik_meldet_kaputte_mock_konfiguration() {
        // Kein stiller Rückfall auf den echten Client bei kaputtem Mock.
        mit_env(&[(ENV_MOCK, "/gibt/es/nicht.json")], || {
            assert!(matches!(
                crate::ai::make_llm(&crate::config::Settings::default()),
                Err(AiError::Config(_))
            ));
        });
    }
}
