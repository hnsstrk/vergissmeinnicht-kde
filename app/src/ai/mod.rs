//! KI-Modul (Spec §4.1/§4.2) — Qt-frei und ohne Bridge-Abhängigkeiten, damit
//! alles hier ohne Qt-Laufzeit testbar ist (wie `parsers.rs`).
//!
//! # Bridge-Kontrakt (Story AI-A3) — konsumiert von den Stufen B1/B2/C/D/E
//!
//! * **Properties am `AppContainer`**, publiziert als reine Property-Sets,
//!   nie über `apply()` — KI-Statusänderungen erzwingen keinen Model-Reset:
//!   - `aiConfigured` — Basis-URL und Modellname gesetzt (UI-Gate: ohne
//!     Konfiguration bleiben alle KI-Bedienelemente versteckt, Spec §3.2).
//!   - `aiBusy` — eine Anfrage läuft (Spinner).
//!   - `aiError` — eigener Fehlerkanal; der globale `errorMessage` bleibt
//!     Sync und Mutationen vorbehalten (Spec §3.4).
//!   - `aiResponseJson` — das validierte JSON-Objekt der jüngsten
//!     abgeschlossenen Anfrage; veraltete oder abgebrochene Ergebnisse
//!     lassen es unverändert.
//!   - `dictationAvailable` — konstant `false`, bis die Startup-Sonde aus
//!     Story AI-A5 sie füllt.
//! * **Worker**: [`starte_anfrage`] folgt dem `start_sync`-Muster — Thread
//!   spawnen, blockierender Call im Worker, Ergebnis über
//!   `qt_thread().queue(...)` zurück auf den Qt-Thread.
//! * **Abbruch/Re-Entranz**: [`Generationen`] statt einer Sperre — die
//!   jüngste Anfrage gewinnt, ältere Ergebnisse werden verworfen (im Worker
//!   UND nochmal im Queue-Callback geprüft). Das Invokable `cancelAiRequest`
//!   erhöht nur den Zähler; der Worker-Thread läuft bis zum Client-Timeout
//!   weiter, sein Ergebnis verfällt.
//! * **So dockt eine Feature-Story an**: eigenes Invokable (Prompt-Aufbau)
//!   → [`starte_anfrage`] → im Queue-Callback nach `ist_aktuell`-Prüfung ins
//!   eigene Result-Property publizieren. Referenz-Implementierung:
//!   `start_ai_request` in `bridge.rs`.

pub mod client;
pub mod mock;
pub mod prompts;
pub mod types;

use client::{AiError, Llm};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Fabrik-Weiche (Spec §8): Bei gesetztem `VMN_AI_MOCK` liefert sie den
/// Konserven-Mock, sonst den echten HTTP-Client aus den Einstellungen.
/// Die eine Stelle, an der die Bridge andockt. `Send + Sync`, weil mehrere
/// Worker-Threads gleichzeitig Anfragen halten können.
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

/// Prozessweiter Llm-Halter: baut den Client beim ersten Zugriff über
/// [`make_llm`] und teilt ihn danach über alle Anfragen. Nötig für die
/// Konserven-Semantik des Mocks (Antworten in Aufruf-Reihenfolge über den
/// App-Lauf, nicht je Anfrage) — und erspart dem echten Client den
/// Secret-Service-Roundtrip je Anfrage. Ein Fehler beim Bauen wird nicht
/// gecacht; der nächste Zugriff versucht es erneut. Bei jeder Änderung der
/// KI-Konfiguration (Provider, Basis-URL, Modell, API-Key) ruft die Bridge
/// [`LlmHalter::invalidiere`] — sonst bediente der alte Client weiter
/// (Story AI-A4).
#[derive(Default)]
pub struct LlmHalter {
    llm: Mutex<Option<Arc<dyn Llm + Send + Sync>>>,
}

impl LlmHalter {
    /// Liefert den geteilten Client; beim ersten Aufruf wird er gebaut.
    /// Wird bewusst im Worker-Thread aufgerufen — der API-Key-Zugriff auf
    /// den Secret Service ist ein D-Bus-Roundtrip und gehört nicht auf den
    /// Qt-Thread.
    pub fn hole(
        &self,
        settings: &crate::config::Settings,
    ) -> Result<Arc<dyn Llm + Send + Sync>, AiError> {
        let mut slot = self.llm.lock().unwrap_or_else(|vergiftet| vergiftet.into_inner());
        if let Some(llm) = &*slot {
            return Ok(Arc::clone(llm));
        }
        let neu: Arc<dyn Llm + Send + Sync> = Arc::from(make_llm(settings)?);
        *slot = Some(Arc::clone(&neu));
        Ok(neu)
    }

    /// Verwirft den gecachten Client — der nächste [`LlmHalter::hole`] baut
    /// aus den dann gültigen Einstellungen (und dem dann gültigen API-Key)
    /// einen neuen. Laufende Anfragen behalten ihren Arc und laufen mit dem
    /// alten Client zu Ende; ihr Ergebnis verfällt ohnehin über den
    /// Generationszähler, wenn es niemand mehr erwartet.
    pub fn invalidiere(&self) {
        let mut slot = self.llm.lock().unwrap_or_else(|vergiftet| vergiftet.into_inner());
        *slot = None;
    }
}

/// Generationszähler für den Abbruch veralteter KI-Anfragen (Spec §4.2):
/// jede Anfrage speichert ihre Generation; „Abbrechen" und jede neuere
/// Anfrage erhöhen den Zähler, veraltete Ergebnisse werden beim Melden
/// verworfen. Bewusst keine Sperre — die jüngste Anfrage gewinnt.
#[derive(Default)]
pub struct Generationen {
    zaehler: AtomicU64,
}

impl Generationen {
    /// Startet eine neue Generation (macht alle laufenden veraltet) und
    /// liefert ihre Nummer.
    pub fn naechste(&self) -> u64 {
        self.zaehler.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Ist `generation` noch die jüngste?
    pub fn ist_aktuell(&self, generation: u64) -> bool {
        self.zaehler.load(Ordering::SeqCst) == generation
    }

    /// Abbruch: macht die laufende Anfrage veraltet, ohne eine neue zu
    /// starten.
    pub fn verwerfen(&self) {
        self.zaehler.fetch_add(1, Ordering::SeqCst);
    }
}

/// Startet eine KI-Anfrage im Worker-Thread (`start_sync`-Muster) und
/// liefert ihre Generationsnummer. `melde` wird höchstens einmal aufgerufen
/// — und nur, wenn die Anfrage beim Eintreffen des Ergebnisses noch die
/// jüngste ist; überholte Ergebnisse verfallen wortlos (Stale-Drop).
///
/// Die Bridge übergibt als `melde` einen Callback, der das Ergebnis per
/// `qt_thread().queue(...)` auf den Qt-Thread bringt und dort mit der
/// mitgelieferten Generation ein zweites Mal [`Generationen::ist_aktuell`]
/// prüft — zwischen Worker-Meldung und Queue-Ausführung kann ein Abbruch
/// oder eine neuere Anfrage dazwischenkommen.
pub fn starte_anfrage<F>(
    generationen: &Arc<Generationen>,
    llm: &Arc<LlmHalter>,
    settings: crate::config::Settings,
    nachrichten: Vec<client::ChatMessage>,
    melde: F,
) -> u64
where
    F: FnOnce(u64, Result<serde_json::Value, AiError>) + Send + 'static,
{
    let generation = generationen.naechste();
    let generationen = Arc::clone(generationen);
    let llm = Arc::clone(llm);
    std::thread::spawn(move || {
        let ergebnis = llm
            .hole(&settings)
            .and_then(|llm| llm.complete_json(&nachrichten));
        if generationen.ist_aktuell(generation) {
            melde(generation, ergebnis);
        }
    });
    generation
}

/// Holt die Modellliste des Endpunkts (`/v1/models`) im Worker-Thread —
/// gleicher Kontrakt wie [`starte_anfrage`]: Generationszähler, Stale-Drop,
/// `melde` höchstens einmal und nur für die jüngste Anfrage. Grundlage der
/// Modellauswahl und des „Speichern und testen"-Checks der Einstellungsseite
/// (Story AI-A4).
pub fn starte_modellliste<F>(
    generationen: &Arc<Generationen>,
    llm: &Arc<LlmHalter>,
    settings: crate::config::Settings,
    melde: F,
) -> u64
where
    F: FnOnce(u64, Result<Vec<String>, AiError>) + Send + 'static,
{
    let generation = generationen.naechste();
    let generationen = Arc::clone(generationen);
    let llm = Arc::clone(llm);
    std::thread::spawn(move || {
        let ergebnis = llm.hole(&settings).and_then(|llm| llm.list_models());
        if generationen.ist_aktuell(generation) {
            melde(generation, ergebnis);
        }
    });
    generation
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::ChatMessage;
    use mock::tests::{mit_env, schreibe_konserven};
    use mock::{ENV_MOCK, ENV_MOCK_DELAY};
    use std::sync::mpsc;
    use std::time::Duration;

    // ─── Generationszähler ──────────────────────────────────────────────────

    #[test]
    fn generationen_juengste_gewinnt() {
        let g = Generationen::default();
        let erste = g.naechste();
        assert!(g.ist_aktuell(erste));
        let zweite = g.naechste();
        // Eine neuere Anfrage macht die ältere veraltet.
        assert!(!g.ist_aktuell(erste));
        assert!(g.ist_aktuell(zweite));
        // Abbruch macht auch die jüngste veraltet, ohne neue zu starten.
        g.verwerfen();
        assert!(!g.ist_aktuell(zweite));
    }

    // ─── Worker + Stale-Drop (Kern von AI-A3, über den AI-B3a-Mock) ─────────

    /// Startet eine Anfrage, deren Meldung mit `marker` etikettiert im Kanal
    /// landet — so ist nachweisbar, WELCHE Anfrage gemeldet hat.
    fn anfrage_mit_marker(
        generationen: &Arc<Generationen>,
        llm: &Arc<LlmHalter>,
        marker: &'static str,
        sender: &mpsc::Sender<(&'static str, u64, Result<serde_json::Value, AiError>)>,
    ) -> u64 {
        let sender = sender.clone();
        starte_anfrage(
            generationen,
            llm,
            crate::config::Settings::default(),
            vec![ChatMessage::user(marker)],
            move |generation, ergebnis| {
                let _ = sender.send((marker, generation, ergebnis));
            },
        )
    }

    #[test]
    fn llm_halter_teilt_eine_instanz_ueber_anfragen() {
        // Die Konserven-Reihenfolge gilt über den App-Lauf: zwei getrennte
        // `hole`-Aufrufe müssen dieselbe Mock-Instanz liefern, sonst zöge
        // jede Anfrage wieder die erste Konserve.
        let datei =
            schreibe_konserven(r#"[{"content": {"n": 1}}, {"content": {"n": 2}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad)], || {
            let halter = LlmHalter::default();
            let einstellungen = crate::config::Settings::default();
            let erste = halter.hole(&einstellungen).unwrap();
            let zweite = halter.hole(&einstellungen).unwrap();
            assert_eq!(erste.complete_json(&[]).unwrap()["n"], 1);
            assert_eq!(zweite.complete_json(&[]).unwrap()["n"], 2);
        });
    }

    #[test]
    fn invalidieren_baut_neuen_client() {
        // Kern der AI-A4-Invalidierung: Nach `invalidiere` liefert der
        // nächste `hole` einen NEUEN Client. Nachweis über die
        // Konserven-Position — ein neuer Mock beginnt wieder bei der ersten
        // Konserve, der alte hätte die zweite serviert.
        let datei =
            schreibe_konserven(r#"[{"content": {"n": 1}}, {"content": {"n": 2}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad)], || {
            let halter = LlmHalter::default();
            let einstellungen = crate::config::Settings::default();
            assert_eq!(halter.hole(&einstellungen).unwrap().complete_json(&[]).unwrap()["n"], 1);
            halter.invalidiere();
            assert_eq!(
                halter.hole(&einstellungen).unwrap().complete_json(&[]).unwrap()["n"],
                1,
                "nach invalidiere muss ein frisch gebauter Client antworten"
            );
        });
    }

    #[test]
    fn modellliste_meldet_ueber_worker() {
        // AI-A4: Modelllisten-Worker über den Mock — meldet die Liste des
        // Traits (`vmn-mock`), ohne eine Konserve zu verbrauchen.
        let datei = schreibe_konserven(r#"[{"content": {"n": 1}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad)], || {
            let generationen = Arc::new(Generationen::default());
            let llm = Arc::new(LlmHalter::default());
            let (sender, empfaenger) = mpsc::channel();
            starte_modellliste(
                &generationen,
                &llm,
                crate::config::Settings::default(),
                move |generation, ergebnis| {
                    let _ = sender.send((generation, ergebnis));
                },
            );
            let (_, ergebnis) = empfaenger
                .recv_timeout(Duration::from_secs(5))
                .expect("Modellliste muss melden");
            assert_eq!(ergebnis.unwrap(), vec!["vmn-mock".to_string()]);
        });
    }

    #[test]
    fn stale_drop_nur_die_juengste_anfrage_meldet() {
        // Zwei Anfragen über die Mock-Latenz (VMN_AI_MOCK_DELAY_MS): beide
        // Konserven sind gleich langsam, die zweite Anfrage startet aber
        // Mikrosekunden nach der ersten und macht sie sofort veraltet —
        // lange bevor irgendein Worker fertig ist. Nur die jüngste darf
        // melden, egal welcher Thread welche Konserve zieht.
        let datei = schreibe_konserven(r#"[{"content": {"antwort": "konserve"}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad), (ENV_MOCK_DELAY, "300")], || {
            let generationen = Arc::new(Generationen::default());
            let llm = Arc::new(LlmHalter::default());
            let (sender, empfaenger) = mpsc::channel();
            let veraltet = anfrage_mit_marker(&generationen, &llm, "veraltet", &sender);
            let aktuell = anfrage_mit_marker(&generationen, &llm, "aktuell", &sender);
            assert!(aktuell > veraltet);

            let (marker, generation, ergebnis) = empfaenger
                .recv_timeout(Duration::from_secs(5))
                .expect("die jüngste Anfrage muss melden");
            assert_eq!(marker, "aktuell");
            assert_eq!(generation, aktuell);
            assert_eq!(ergebnis.unwrap()["antwort"], "konserve");

            // Die veraltete Anfrage meldet nie — auch nicht nach Ablauf
            // ihrer eigenen Latenz (300 ms, hier großzügig überwartet).
            assert!(empfaenger.recv_timeout(Duration::from_millis(700)).is_err());
        });
    }

    #[test]
    fn verwerfen_unterdrueckt_laufende_anfrage() {
        // Abbruch (cancelAiRequest-Pfad): Zähler erhöhen, Ergebnis verfällt.
        let datei = schreibe_konserven(r#"[{"content": {"antwort": "zu spät"}}]"#);
        let pfad = datei.path().to_str().unwrap().to_string();
        mit_env(&[(ENV_MOCK, &pfad), (ENV_MOCK_DELAY, "200")], || {
            let generationen = Arc::new(Generationen::default());
            let llm = Arc::new(LlmHalter::default());
            let (sender, empfaenger) = mpsc::channel();
            anfrage_mit_marker(&generationen, &llm, "abgebrochen", &sender);
            generationen.verwerfen();
            assert!(empfaenger.recv_timeout(Duration::from_millis(700)).is_err());
        });
    }

    #[test]
    fn anfrage_meldet_fabrik_fehler() {
        // Kaputte Mock-Konfiguration erreicht die Meldung als Config-Fehler
        // (kein stiller Rückfall, kein verschluckter Fehler im Worker).
        mit_env(&[(ENV_MOCK, "/gibt/es/nicht.json")], || {
            let generationen = Arc::new(Generationen::default());
            let llm = Arc::new(LlmHalter::default());
            let (sender, empfaenger) = mpsc::channel();
            anfrage_mit_marker(&generationen, &llm, "fehler", &sender);
            let (_, _, ergebnis) = empfaenger
                .recv_timeout(Duration::from_secs(5))
                .expect("Fehler muss gemeldet werden");
            assert!(matches!(ergebnis, Err(AiError::Config(_))));
        });
    }
}
