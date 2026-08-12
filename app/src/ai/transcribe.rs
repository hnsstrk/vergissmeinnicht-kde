//! Diktat (Story AI-A5, Spec §4.1): Aufnahme über `pw-record` und
//! Spracherkennung über eines von zwei Whisper-Backends, beide als
//! Unterprozesse mit eigenem JSON-Ausgabeformat. Qt-frei wie das übrige
//! `ai`-Modul — alles hier ist ohne Qt-Laufzeit testbar.
//!
//! # Warum kein hartes Kill
//!
//! `pw-record` schreibt die Größenfelder des RIFF-Headers erst beim
//! geordneten Ende. Gemessen am 12.08.2026 auf der Referenzmaschine: nach
//! SIGINT steht im Größenfeld 93506 (die tatsächliche Datenmenge), nach
//! SIGKILL bleibt es auf dem Platzhalter 8 stehen — die Datei sieht auf den
//! ersten Blick gleich aus, und der Fehler fällt erst im Whisper-Lauf als
//! „leere Aufnahme" auf. Deshalb: SIGINT, warten, notfalls SIGTERM, warten.
//! SIGKILL nur als allerletzte Rettung beim Aufräumen ([`Aufnahme::drop`]),
//! wo die Datei ohnehin verworfen wird — kein `pw-record` darf die App
//! überleben.
//!
//! # Besitz und Aufräumen
//!
//! [`Aufnahme`] besitzt den Kindprozess UND die WAV-Datei. Wer den Wert
//! fallen lässt — regulär, beim Schließen des Fensters oder beim Abwickeln
//! einer Panik — beendet damit die Aufnahme und löscht die Datei. Das
//! Transkript des Backends räumt [`Aufraeumer`] weg, auch im Fehlerfall.

use crate::config::Settings;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Aufnahmeprogramm. PipeWire ist auf jedem aktuellen Linux-Desktop da —
/// das erspart eine Abhängigkeit auf qt6-multimedia (Spec §4.1).
pub const PW_RECORD: &str = "pw-record";

/// CLI des Backends `openai-whisper`, erwartet im PATH.
pub const WHISPER_CLI: &str = "whisper";

/// Diktiersprache beider Backends (Spec §4.1).
const SPRACHE: &str = "de";

/// Aufnahmeformat: 16 kHz, mono, s16 — genau das, was beide Whisper-Backends
/// intern verarbeiten. Alles andere resampeln sie ohnehin wieder herunter.
const ABTASTRATE: &str = "16000";

/// Vorgabe-Modell des openai-whisper-Backends, falls das Feld leer ist
/// (gleicher Wert wie `Settings::default`).
const WHISPER_MODELL_VORGABE: &str = "small";

/// Wartefrist je Beendigungssignal. `pw-record` beendet sich in der Praxis
/// binnen weniger Millisekunden; die Frist ist die Obergrenze, nicht die
/// Erwartung.
const SIGNAL_FRIST: Duration = Duration::from_secs(2);

/// Verfügbares Spracherkennungs-Backend (Konfigurationsfeld
/// `ai_stt_backend`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttBackend {
    /// `whisper` aus dem PATH (CPU, Modelle laden sich bei Bedarf nach).
    OpenAiWhisper,
    /// Selbst gebautes `whisper-cli` plus GGML-Modelldatei (beides als Pfad
    /// konfiguriert) — der Weg für GPU-Builds.
    WhisperCpp,
}

impl SttBackend {
    /// Backend aus dem Konfigurationswert. Unbekannte Namen liefern `None` —
    /// die Sonde versteckt das Mikrofon dann, statt zu raten.
    pub fn aus_config(wert: &str) -> Option<Self> {
        match wert.trim() {
            "openai-whisper" => Some(Self::OpenAiWhisper),
            "whisper-cpp" => Some(Self::WhisperCpp),
            _ => None,
        }
    }
}

/// Was der Diktier-Kette fehlt. Trägt den geprüften Namen bzw. Pfad mit,
/// damit die Meldung sagt, wonach gesucht wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fehlend {
    /// Kein `pw-record` im PATH — ohne Aufnahme kein Diktat.
    Aufnahmeprogramm,
    /// `ai_stt_backend` trägt einen Namen, den es nicht gibt.
    UnbekanntesBackend(String),
    /// Whisper-Programm fehlt (PATH-Name beim openai-whisper-Backend,
    /// konfigurierter Pfad bei whisper.cpp).
    Whisperprogramm(String),
    /// GGML-Modelldatei des whisper.cpp-Backends fehlt.
    Modelldatei(String),
    /// Das Laufzeitverzeichnis für Aufnahmen und Transkripte lässt sich
    /// nicht anlegen oder nicht beschreiben.
    Laufzeitverzeichnis(String),
}

impl std::fmt::Display for Fehlend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fehlend::Aufnahmeprogramm => {
                write!(f, "Diktat nicht möglich: {PW_RECORD} nicht gefunden")
            }
            Fehlend::UnbekanntesBackend(name) => {
                write!(f, "Diktat nicht möglich: unbekanntes Spracherkennungs-Backend „{name}“")
            }
            Fehlend::Whisperprogramm(pfad) => {
                write!(f, "Diktat nicht möglich: Whisper-Programm „{pfad}“ nicht gefunden")
            }
            Fehlend::Modelldatei(pfad) => {
                write!(f, "Diktat nicht möglich: Modelldatei „{pfad}“ nicht gefunden")
            }
            Fehlend::Laufzeitverzeichnis(pfad) => {
                write!(f, "Diktat nicht möglich: Laufzeitverzeichnis „{pfad}“ nicht beschreibbar")
            }
        }
    }
}

/// Fehler der Diktier-Kette. Eigene Meldungen auf Deutsch wie bei
/// [`crate::ai::client::AiError`]; die Bridge reicht sie in den KI-eigenen
/// Fehlerkanal `aiError` durch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranskriptFehler {
    /// Etwas ließ sich nicht starten (Programm weg, Rechte, Laufzeitpfad).
    Start(String),
    /// Die Aufnahme ließ sich nicht sauber beenden.
    Aufnahme(String),
    /// Das Backend lief, meldete aber einen Fehlerstatus.
    Backend(String),
    /// Ausgabedatei fehlt oder trägt nicht das erwartete Format.
    Ausgabe(String),
    /// Sauberer Lauf, aber kein Wort erkannt.
    Leer,
    /// Die Voraussetzungen fehlen (dieselbe Prüfung wie die Startsonde).
    NichtVerfuegbar(Fehlend),
}

impl std::fmt::Display for TranskriptFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranskriptFehler::Start(e) => write!(f, "Diktat lässt sich nicht starten: {e}"),
            TranskriptFehler::Aufnahme(e) => write!(f, "Aufnahme: {e}"),
            TranskriptFehler::Backend(e) => write!(f, "Spracherkennung: {e}"),
            TranskriptFehler::Ausgabe(e) => write!(f, "Ausgabe der Spracherkennung: {e}"),
            TranskriptFehler::Leer => write!(f, "Keine Sprache erkannt."),
            TranskriptFehler::NichtVerfuegbar(fehlend) => write!(f, "{fehlend}"),
        }
    }
}

// ─── Verfügbarkeitsprüfung ──────────────────────────────────────────────────

/// Startsonde (Spec §4.1): Ist die ganze Kette da? Speist die
/// Bridge-Eigenschaft `dictationAvailable`. Datei- und PATH-Prüfung plus ein
/// Schreibtest auf das Laufzeitverzeichnis, kein Prozessstart — läuft billig
/// genug für den Qt-Thread.
pub fn verfuegbarkeit(settings: &Settings) -> Result<(), Fehlend> {
    verfuegbarkeit_mit(settings, &im_pfad_ausfuehrbar, &|pfad| ist_ausfuehrbar(pfad), &|| {
        laufzeit_beschreibbar(&laufzeit_verzeichnis())
    })
}

/// Prüflogik mit austauschbarer Umgebung — so testen die Negativfälle ohne
/// echte Installation.
fn verfuegbarkeit_mit(
    settings: &Settings,
    im_pfad: &dyn Fn(&str) -> bool,
    ausfuehrbar: &dyn Fn(&Path) -> bool,
    laufzeit_schreibbar: &dyn Fn() -> bool,
) -> Result<(), Fehlend> {
    if !im_pfad(PW_RECORD) {
        return Err(Fehlend::Aufnahmeprogramm);
    }
    let backend = SttBackend::aus_config(&settings.ai_stt_backend).ok_or_else(|| {
        Fehlend::UnbekanntesBackend(settings.ai_stt_backend.trim().to_string())
    })?;
    match backend {
        SttBackend::OpenAiWhisper => {
            if !im_pfad(WHISPER_CLI) {
                return Err(Fehlend::Whisperprogramm(WHISPER_CLI.into()));
            }
        }
        SttBackend::WhisperCpp => {
            let programm = settings.ai_whisper_cpp_binary.trim();
            if programm.is_empty() || !ausfuehrbar(Path::new(programm)) {
                return Err(Fehlend::Whisperprogramm(programm.to_string()));
            }
            let modell = settings.ai_whisper_cpp_model.trim();
            // Die Modelldatei wird nur gelesen — Ausführbarkeit wäre hier das
            // falsche Kriterium.
            if modell.is_empty() || !Path::new(modell).is_file() {
                return Err(Fehlend::Modelldatei(modell.to_string()));
            }
        }
    }
    // Ohne beschreibbares Laufzeitverzeichnis gibt es keine Aufnahme — dann
    // gehört das Mikrofon versteckt, nicht erst der Aufnahmestart gescheitert.
    if !laufzeit_schreibbar() {
        return Err(Fehlend::Laufzeitverzeichnis(
            laufzeit_verzeichnis().display().to_string(),
        ));
    }
    Ok(())
}

/// Schreibtest: Verzeichnis anlegen (wie [`Aufnahme::starte`] es tut), eine
/// Probedatei erzeugen und sofort wieder entfernen — es bleibt nichts liegen.
/// `create_dir_all` allein reicht nicht: Ein bereits existierendes, aber
/// schreibgeschütztes Verzeichnis meldet dort keinen Fehler.
fn laufzeit_beschreibbar(verzeichnis: &Path) -> bool {
    if std::fs::create_dir_all(verzeichnis).is_err() {
        return false;
    }
    let probe = verzeichnis.join(format!(".sonde-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(datei) => {
            drop(datei);
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Liegt `name` als ausführbare Datei in einem PATH-Verzeichnis?
fn im_pfad_ausfuehrbar(name: &str) -> bool {
    let Some(pfad) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&pfad).any(|dir| ist_ausfuehrbar(&dir.join(name)))
}

fn ist_ausfuehrbar(pfad: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(pfad)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

// ─── Aufnahme ───────────────────────────────────────────────────────────────

/// Laufender `pw-record`-Prozess samt seiner WAV-Datei. Solange dieser Wert
/// lebt, läuft die Aufnahme; [`Aufnahme::stoppe`] beendet sie geordnet,
/// `Drop` räumt in jedem Fall auf (Fensterschluss, Panik, vergessener Stopp).
pub struct Aufnahme {
    /// `None`, sobald der Prozess abgeräumt (gewartet) ist.
    kind: Option<Child>,
    wav: PathBuf,
}

/// Laufende Nummer für eindeutige Dateinamen innerhalb eines App-Laufs.
static NUMMER: AtomicU64 = AtomicU64::new(0);

/// Verzeichnis für Aufnahmen und Transkripte: XDG-Laufzeitverzeichnis
/// (`/run/user/<uid>`, wird beim Abmelden geleert). Ohne gesetztes
/// Laufzeitverzeichnis das temporäre Verzeichnis.
pub fn laufzeit_verzeichnis() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("vergissmeinnicht")
}

impl Aufnahme {
    /// Startet `pw-record` in das Laufzeitverzeichnis. Der Prozess läuft, bis
    /// [`Aufnahme::stoppe`] ihn beendet — oder bis dieser Wert stirbt.
    pub fn starte() -> Result<Self, TranskriptFehler> {
        let verzeichnis = laufzeit_verzeichnis();
        std::fs::create_dir_all(&verzeichnis).map_err(|e| {
            TranskriptFehler::Start(format!("{}: {e}", verzeichnis.display()))
        })?;
        let wav = verzeichnis.join(format!(
            "diktat-{}-{}.wav",
            std::process::id(),
            NUMMER.fetch_add(1, Ordering::SeqCst)
        ));
        let kind = Command::new(PW_RECORD)
            .arg("--rate")
            .arg(ABTASTRATE)
            .arg("--channels")
            .arg("1")
            .arg("--format")
            .arg("s16")
            .arg(&wav)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| TranskriptFehler::Start(format!("{PW_RECORD}: {e}")))?;
        Ok(Self { kind: Some(kind), wav })
    }

    /// Pfad der aufgenommenen Datei — gültig, sobald [`Aufnahme::stoppe`]
    /// erfolgreich war.
    pub fn wav(&self) -> &Path {
        &self.wav
    }

    /// Beendet die Aufnahme geordnet und wartet den Prozess ab (siehe
    /// Modulkommentar: hart abgeschossen wäre das WAV unbrauchbar).
    /// Mehrfachaufrufe sind folgenlos.
    pub fn stoppe(&mut self) -> Result<(), TranskriptFehler> {
        let Some(mut kind) = self.kind.take() else {
            return Ok(());
        };
        match beende_freundlich(&mut kind) {
            Ok(()) => Ok(()),
            // Prozess zurücklegen: `Drop` bekommt die letzte Rettung.
            Err(e) => {
                self.kind = Some(kind);
                Err(e)
            }
        }
    }
}

impl Drop for Aufnahme {
    fn drop(&mut self) {
        // Regulärer Weg zuerst — auch beim Fensterschluss soll SIGINT
        // greifen und nicht das Beil.
        let _ = self.stoppe();
        if let Some(kind) = &mut self.kind {
            // Allerletzte Rettung: Ein `pw-record`, der die App überlebt, ist
            // schlimmer als eine unbrauchbare Datei — und die Datei wird hier
            // ohnehin gelöscht.
            let _ = kind.kill();
            let _ = kind.wait();
        }
        let _ = std::fs::remove_file(&self.wav);
    }
}

/// SIGINT, warten, notfalls SIGTERM, warten. Kein SIGKILL.
fn beende_freundlich(kind: &mut Child) -> Result<(), TranskriptFehler> {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        if ist_beendet(kind)? {
            return Ok(());
        }
        sende_signal(kind, signal)?;
        if warte_auf_ende(kind, SIGNAL_FRIST)? {
            return Ok(());
        }
    }
    Err(TranskriptFehler::Aufnahme(format!(
        "{PW_RECORD} reagiert weder auf SIGINT noch auf SIGTERM"
    )))
}

fn ist_beendet(kind: &mut Child) -> Result<bool, TranskriptFehler> {
    kind.try_wait()
        .map(|status| status.is_some())
        .map_err(|e| TranskriptFehler::Aufnahme(e.to_string()))
}

fn sende_signal(kind: &Child, signal: i32) -> Result<(), TranskriptFehler> {
    // Sicher: Die PID stammt aus einem noch nicht abgewarteten Kindprozess.
    // Bis zum `wait` bleibt sie reserviert und kann keinen fremden Prozess
    // treffen.
    let rc = unsafe { libc::kill(kind.id() as libc::pid_t, signal) };
    if rc == 0 {
        Ok(())
    } else {
        Err(TranskriptFehler::Aufnahme(format!(
            "Signal {signal} an {PW_RECORD}: {}",
            std::io::Error::last_os_error()
        )))
    }
}

/// Wartet bis zur Frist auf das Ende; `true`, wenn der Prozess abgeräumt ist.
fn warte_auf_ende(kind: &mut Child, frist: Duration) -> Result<bool, TranskriptFehler> {
    let ende = Instant::now() + frist;
    loop {
        if ist_beendet(kind)? {
            return Ok(true);
        }
        if Instant::now() >= ende {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ─── Spracherkennung ────────────────────────────────────────────────────────

/// Löscht seine Datei, sobald er stirbt — für das Transkript, das im
/// Laufzeitverzeichnis liegt und auch bei Fehlern nicht liegen bleiben darf.
struct Aufraeumer(PathBuf);

impl Drop for Aufraeumer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Transkribiert eine fertige Aufnahme mit dem konfigurierten Backend.
/// Blockiert (Modell laden dauert Sekunden) — gehört in einen Worker-Thread.
/// Die WAV-Datei bleibt Sache des Aufrufers, das Transkript räumt diese
/// Funktion selbst weg.
pub fn transkribiere(settings: &Settings, wav: &Path) -> Result<String, TranskriptFehler> {
    verfuegbarkeit(settings).map_err(TranskriptFehler::NichtVerfuegbar)?;
    let backend = SttBackend::aus_config(&settings.ai_stt_backend).ok_or_else(|| {
        TranskriptFehler::NichtVerfuegbar(Fehlend::UnbekanntesBackend(
            settings.ai_stt_backend.trim().to_string(),
        ))
    })?;
    // Beide Backends legen ihre Ausgabe neben die Aufnahme, unter demselben
    // Namensstamm — siehe `kommando`.
    let transkript = wav.with_extension("json");
    let _aufraeumer = Aufraeumer(transkript.clone());
    let ausgabe = kommando(backend, settings, wav)
        .output()
        .map_err(|e| TranskriptFehler::Start(format!("{}: {e}", programmname(backend, settings))))?;
    if !ausgabe.status.success() {
        return Err(TranskriptFehler::Backend(format!(
            "{} endete mit {} — {}",
            programmname(backend, settings),
            ausgabe.status,
            letzte_zeile(&String::from_utf8_lossy(&ausgabe.stderr))
        )));
    }
    let roh = std::fs::read_to_string(&transkript).map_err(|e| {
        TranskriptFehler::Ausgabe(format!("{}: {e}", transkript.display()))
    })?;
    let text = match backend {
        SttBackend::OpenAiWhisper => parse_openai_whisper(&roh)?,
        SttBackend::WhisperCpp => parse_whisper_cpp(&roh)?,
    };
    if text.is_empty() {
        return Err(TranskriptFehler::Leer);
    }
    Ok(text)
}

/// Programmname für Fehlermeldungen.
fn programmname(backend: SttBackend, settings: &Settings) -> String {
    match backend {
        SttBackend::OpenAiWhisper => WHISPER_CLI.to_string(),
        SttBackend::WhisperCpp => settings.ai_whisper_cpp_binary.trim().to_string(),
    }
}

/// Baut den Backend-Aufruf (Spec §4.1). Beide Varianten schreiben ihre
/// JSON-Ausgabe neben die Aufnahme unter deren Namensstamm: openai-whisper
/// über `--output_dir` (Dateiname = Stamm der Eingabe), whisper.cpp über
/// `-of` (Pfad ohne Endung). Getrennt gehalten, damit der Aufbau ohne
/// Prozessstart prüfbar bleibt.
fn kommando(backend: SttBackend, settings: &Settings, wav: &Path) -> Command {
    match backend {
        SttBackend::OpenAiWhisper => {
            let modell = nicht_leer(&settings.ai_whisper_model, WHISPER_MODELL_VORGABE);
            let verzeichnis = wav.parent().unwrap_or(Path::new(".")).to_path_buf();
            let mut cmd = Command::new(WHISPER_CLI);
            cmd.arg("--model")
                .arg(modell)
                .arg("--language")
                .arg(SPRACHE)
                .arg("--output_format")
                .arg("json")
                .arg("--output_dir")
                .arg(verzeichnis)
                .arg(wav);
            cmd
        }
        SttBackend::WhisperCpp => {
            let mut cmd = Command::new(settings.ai_whisper_cpp_binary.trim());
            cmd.arg("-m")
                .arg(settings.ai_whisper_cpp_model.trim())
                .arg("-f")
                .arg(wav)
                .arg("-l")
                .arg(SPRACHE)
                .arg("--output-json")
                .arg("-of")
                .arg(wav.with_extension(""))
                // Ohne `--no-prints` schreibt whisper.cpp den erkannten Text
                // zusätzlich auf stdout; die Ausgabe interessiert uns nur als
                // JSON-Datei.
                .arg("--no-prints");
            cmd
        }
    }
}

fn nicht_leer<'a>(wert: &'a str, vorgabe: &'a str) -> &'a str {
    let getrimmt = wert.trim();
    if getrimmt.is_empty() {
        vorgabe
    } else {
        getrimmt
    }
}

/// Letzte nicht-leere Zeile einer Fehlerausgabe — mehr braucht die Meldung
/// nicht, und beide Backends sind auf stderr geschwätzig.
fn letzte_zeile(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .rfind(|z| !z.is_empty())
        .unwrap_or("keine Fehlerausgabe")
        .to_string()
}

/// Ausgabeformat von `whisper --output_format json`: der vollständige Text
/// steht im Feld `text`, die Segmente daneben (für das Diktat uninteressant).
pub fn parse_openai_whisper(json: &str) -> Result<String, TranskriptFehler> {
    let wert: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| TranskriptFehler::Ausgabe(format!("openai-whisper: {e}")))?;
    let text = wert
        .get("text")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| TranskriptFehler::Ausgabe("openai-whisper: Feld „text“ fehlt".into()))?;
    Ok(normalisiere(text))
}

/// Ausgabeformat von `whisper-cli --output-json`: kein Gesamttext, sondern
/// eine Segmentliste unter `transcription`, jedes Segment mit führendem
/// Leerzeichen im Feld `text`.
pub fn parse_whisper_cpp(json: &str) -> Result<String, TranskriptFehler> {
    let wert: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| TranskriptFehler::Ausgabe(format!("whisper.cpp: {e}")))?;
    let segmente = wert
        .get("transcription")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            TranskriptFehler::Ausgabe("whisper.cpp: Feld „transcription“ fehlt".into())
        })?;
    let text = segmente
        .iter()
        .filter_map(|s| s.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(normalisiere(&text))
}

/// Vereinheitlicht den Weißraum: beide Backends liefern führende Leerzeichen
/// und Zeilenumbrüche je Segment, das Ziel ist aber ein Aufgabentitel.
fn normalisiere(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Echte Ausgaben der Referenzmaschine vom 12.08.2026 (Beleg im Ticket
    /// AI-A5): beide Backends auf `samples/jfk.wav` von whisper.cpp —
    /// openai-whisper mit Modell `base` auf der CPU, whisper.cpp mit
    /// `ggml-large-v3` auf der GPU.
    const AUSGABE_OPENAI: &str = include_str!("fixtures/openai-whisper-jfk.json");
    const AUSGABE_WHISPER_CPP: &str = include_str!("fixtures/whisper-cpp-jfk.json");

    const SATZ: &str = "And so, my fellow Americans, ask not what your country can do for you, \
                        ask what you can do for your country.";

    fn einstellungen(backend: &str) -> Settings {
        Settings { ai_stt_backend: backend.into(), ..Settings::default() }
    }

    // ─── Ausgabe-Parser (echte Fixtures) ────────────────────────────────────

    #[test]
    fn parst_openai_whisper_ausgabe() {
        let text = parse_openai_whisper(AUSGABE_OPENAI).unwrap();
        assert_eq!(text, SATZ);
    }

    #[test]
    fn parst_whisper_cpp_ausgabe() {
        // whisper.cpp liefert drei Segmente ohne Gesamttext — der Parser
        // fügt sie zusammen und normalisiert den Weißraum.
        let text = parse_whisper_cpp(AUSGABE_WHISPER_CPP).unwrap();
        assert_eq!(text, SATZ);
    }

    #[test]
    fn parser_verwechseln_die_formate_nicht() {
        // Jede Ausgabe hat ihr eigenes Format: das Feld des einen Backends
        // gibt es beim anderen nicht.
        assert!(matches!(
            parse_openai_whisper(AUSGABE_WHISPER_CPP),
            Err(TranskriptFehler::Ausgabe(_))
        ));
        assert!(matches!(
            parse_whisper_cpp(AUSGABE_OPENAI),
            Err(TranskriptFehler::Ausgabe(_))
        ));
    }

    #[test]
    fn parser_melden_kaputte_ausgabe() {
        assert!(matches!(
            parse_openai_whisper("{abgeschnitten"),
            Err(TranskriptFehler::Ausgabe(_))
        ));
        assert!(matches!(
            parse_whisper_cpp("{abgeschnitten"),
            Err(TranskriptFehler::Ausgabe(_))
        ));
    }

    #[test]
    fn stille_ergibt_leeren_text() {
        // Beide Backends liefern bei stiller Aufnahme ein gültiges Dokument
        // ohne Wörter — das ist kein Formatfehler, sondern leerer Text.
        assert_eq!(parse_openai_whisper(r#"{"text": "  ", "segments": []}"#).unwrap(), "");
        assert_eq!(parse_whisper_cpp(r#"{"transcription": []}"#).unwrap(), "");
    }

    // ─── Backend-Namen ──────────────────────────────────────────────────────

    #[test]
    fn backend_namen_aus_config() {
        assert_eq!(SttBackend::aus_config("openai-whisper"), Some(SttBackend::OpenAiWhisper));
        assert_eq!(SttBackend::aus_config(" whisper-cpp "), Some(SttBackend::WhisperCpp));
        assert_eq!(SttBackend::aus_config("whisper.cpp"), None);
        assert_eq!(SttBackend::aus_config(""), None);
    }

    // ─── Sonde: Negativfälle ────────────────────────────────────────────────

    /// Prüfung mit vollständig vorhandener Umgebung — die Positivkontrolle,
    /// gegen die die Negativfälle abheben. Das Laufzeitverzeichnis gilt hier
    /// als beschreibbar; seinen Negativfall prüft `sonde_ohne_schreibbares_…`.
    fn sonde(settings: &Settings, vorhanden: &[&str]) -> Result<(), Fehlend> {
        let namen: Vec<String> = vorhanden.iter().map(|s| s.to_string()).collect();
        let im_pfad = |name: &str| namen.iter().any(|n| n == name);
        let ausfuehrbar = |pfad: &Path| {
            namen.iter().any(|n| n.as_str() == pfad.to_string_lossy())
        };
        verfuegbarkeit_mit(settings, &im_pfad, &ausfuehrbar, &|| true)
    }

    #[test]
    fn sonde_gruen_wenn_alles_da_ist() {
        assert_eq!(sonde(&einstellungen("openai-whisper"), &[PW_RECORD, WHISPER_CLI]), Ok(()));
        let mut s = einstellungen("whisper-cpp");
        s.ai_whisper_cpp_binary = "/opt/whisper.cpp/whisper-cli".into();
        // Die Modelldatei muss wirklich existieren — sie wird direkt geprüft.
        let modell = modelldatei();
        s.ai_whisper_cpp_model = modell.path().to_string_lossy().into_owned();
        assert_eq!(sonde(&s, &[PW_RECORD, "/opt/whisper.cpp/whisper-cli"]), Ok(()));
    }

    /// Echte Datei für die Modellprüfung — die prüft `is_file()` direkt, weil
    /// eine Modelldatei nicht ausführbar sein muss.
    fn modelldatei() -> tempfile::NamedTempFile {
        tempfile::NamedTempFile::new().expect("Testdatei")
    }

    #[test]
    fn sonde_ohne_pw_record() {
        // Ohne Aufnahmeprogramm ist alles andere gleichgültig.
        assert_eq!(
            sonde(&einstellungen("openai-whisper"), &[WHISPER_CLI]),
            Err(Fehlend::Aufnahmeprogramm)
        );
    }

    #[test]
    fn sonde_ohne_whisper_im_pfad() {
        assert_eq!(
            sonde(&einstellungen("openai-whisper"), &[PW_RECORD]),
            Err(Fehlend::Whisperprogramm(WHISPER_CLI.into()))
        );
    }

    #[test]
    fn sonde_bei_unbekanntem_backend() {
        assert_eq!(
            sonde(&einstellungen("whisperx"), &[PW_RECORD, WHISPER_CLI]),
            Err(Fehlend::UnbekanntesBackend("whisperx".into()))
        );
    }

    #[test]
    fn sonde_ohne_whisper_cpp_programm() {
        let mut s = einstellungen("whisper-cpp");
        let modell = modelldatei();
        s.ai_whisper_cpp_model = modell.path().to_string_lossy().into_owned();
        // Feld leer gelassen — der häufigste Fall direkt nach dem Umschalten.
        assert_eq!(sonde(&s, &[PW_RECORD]), Err(Fehlend::Whisperprogramm(String::new())));
        s.ai_whisper_cpp_binary = "/gibt/es/nicht/whisper-cli".into();
        assert_eq!(
            sonde(&s, &[PW_RECORD]),
            Err(Fehlend::Whisperprogramm("/gibt/es/nicht/whisper-cli".into()))
        );
    }

    #[test]
    fn sonde_ohne_ggml_modelldatei() {
        let mut s = einstellungen("whisper-cpp");
        s.ai_whisper_cpp_binary = "/opt/whisper.cpp/whisper-cli".into();
        let vorhanden = [PW_RECORD, "/opt/whisper.cpp/whisper-cli"];
        assert_eq!(sonde(&s, &vorhanden), Err(Fehlend::Modelldatei(String::new())));
        s.ai_whisper_cpp_model = "/gibt/es/nicht/ggml-large-v3.bin".into();
        assert_eq!(
            sonde(&s, &vorhanden),
            Err(Fehlend::Modelldatei("/gibt/es/nicht/ggml-large-v3.bin".into()))
        );
    }

    #[test]
    fn sonde_ohne_schreibbares_laufzeitverzeichnis() {
        // Kette vollständig installiert, aber das Laufzeitverzeichnis nicht
        // beschreibbar — genau der Fall, in dem `Aufnahme::starte` scheitern
        // würde. Die Sonde muss das Mikrofon vorher verstecken.
        let ergebnis = verfuegbarkeit_mit(
            &einstellungen("openai-whisper"),
            &|_| true,
            &|_| true,
            &|| false,
        );
        assert!(matches!(ergebnis, Err(Fehlend::Laufzeitverzeichnis(_))));
    }

    #[test]
    fn schreibtest_erkennt_schreibgeschuetztes_verzeichnis() {
        use std::os::unix::fs::PermissionsExt;
        let ordner = tempfile::tempdir().unwrap();
        // Beschreibbar: Probe läuft durch und hinterlässt nichts.
        assert!(laufzeit_beschreibbar(ordner.path()));
        assert_eq!(
            std::fs::read_dir(ordner.path()).unwrap().count(),
            0,
            "der Schreibtest darf keine Datei zurücklassen"
        );
        // Schreibgeschützt (chmod 555): existiert, ist aber nicht nutzbar.
        std::fs::set_permissions(ordner.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!laufzeit_beschreibbar(ordner.path()));
        // Untergeordnetes Verzeichnis unter dem geschützten: `create_dir_all`
        // scheitert schon beim Anlegen — auch das ist „nicht beschreibbar".
        assert!(!laufzeit_beschreibbar(&ordner.path().join("vergissmeinnicht")));
        // Rechte zurück, damit das TempDir sich selbst wegräumen kann.
        std::fs::set_permissions(ordner.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn sonde_nimmt_das_echte_system() {
        // `verfuegbarkeit` darf nie panisch werden, egal was installiert ist
        // — das Ergebnis hängt von der Maschine ab, die Aussage nicht.
        let _ = verfuegbarkeit(&Settings::default());
    }

    // ─── Kommandozeilen ─────────────────────────────────────────────────────

    fn zeile(cmd: &Command) -> String {
        let mut teile = vec![cmd.get_program().to_string_lossy().into_owned()];
        teile.extend(cmd.get_args().map(|a| a.to_string_lossy().into_owned()));
        teile.join(" ")
    }

    #[test]
    fn kommando_openai_whisper() {
        let s = einstellungen("openai-whisper");
        let cmd = kommando(SttBackend::OpenAiWhisper, &s, Path::new("/run/vmn/diktat-1.wav"));
        assert_eq!(
            zeile(&cmd),
            "whisper --model small --language de --output_format json \
             --output_dir /run/vmn /run/vmn/diktat-1.wav"
        );
    }

    #[test]
    fn kommando_openai_whisper_faellt_auf_vorgabemodell_zurueck() {
        let mut s = einstellungen("openai-whisper");
        s.ai_whisper_model = "  ".into();
        let cmd = kommando(SttBackend::OpenAiWhisper, &s, Path::new("/run/vmn/diktat-1.wav"));
        assert!(zeile(&cmd).contains("--model small"));
    }

    #[test]
    fn kommando_whisper_cpp() {
        let mut s = einstellungen("whisper-cpp");
        s.ai_whisper_cpp_binary = "/opt/whisper.cpp/whisper-cli".into();
        s.ai_whisper_cpp_model = "/opt/whisper.cpp/ggml-large-v3.bin".into();
        let cmd = kommando(SttBackend::WhisperCpp, &s, Path::new("/run/vmn/diktat-1.wav"));
        // `-of` ohne Endung: whisper.cpp hängt `.json` an — dieselbe Datei,
        // die `transkribiere` danach liest und löscht.
        assert_eq!(
            zeile(&cmd),
            "/opt/whisper.cpp/whisper-cli -m /opt/whisper.cpp/ggml-large-v3.bin \
             -f /run/vmn/diktat-1.wav -l de --output-json -of /run/vmn/diktat-1 --no-prints"
        );
    }

    #[test]
    fn transkript_pfad_passt_zum_kommando() {
        // Beide Backends schreiben nach `<Stamm>.json` — genau den Pfad
        // liest und löscht `transkribiere`.
        let wav = Path::new("/run/vmn/diktat-1.wav");
        assert_eq!(wav.with_extension("json"), Path::new("/run/vmn/diktat-1.json"));
    }

    // ─── Freundliches Beenden ───────────────────────────────────────────────

    #[test]
    fn sigint_beendet_und_wartet_ab() {
        // Stellvertreter statt echter Aufnahme: `sleep` stirbt an SIGINT wie
        // pw-record. Geprüft wird der Signalweg samt Abwarten — nach
        // `beende_freundlich` ist der Prozess abgeräumt (kein Zombie).
        let mut kind = Command::new("sleep").arg("60").spawn().expect("sleep startbar");
        let pid = kind.id();
        beende_freundlich(&mut kind).expect("SIGINT muss reichen");
        assert!(ist_beendet(&mut kind).unwrap(), "Prozess muss abgewartet sein");
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "nach dem Abwarten darf kein Prozesseintrag mehr existieren"
        );
    }

    #[test]
    fn sigterm_greift_wenn_sigint_ignoriert_wird() {
        // `sh` mit SIGINT-Falle: erst SIGTERM beendet ihn. Belegt die zweite
        // Stufe der Beendigungskette.
        let mut kind = Command::new("sh")
            .arg("-c")
            .arg("trap '' INT; sleep 60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sh startbar");
        beende_freundlich(&mut kind).expect("SIGTERM muss greifen");
        assert!(ist_beendet(&mut kind).unwrap());
    }

    #[test]
    fn beenden_eines_toten_prozesses_ist_folgenlos() {
        let mut kind = Command::new("true").spawn().expect("true startbar");
        // Erst abwarten lassen, dann beenden — darf nicht auf einen fremden
        // Prozess zielen und nicht scheitern.
        std::thread::sleep(Duration::from_millis(50));
        beende_freundlich(&mut kind).expect("bereits beendet ist kein Fehler");
        beende_freundlich(&mut kind).expect("zweiter Aufruf ebenso");
    }

    // ─── Aufräumen ──────────────────────────────────────────────────────────

    #[test]
    fn aufraeumer_loescht_seine_datei() {
        let ordner = tempfile::tempdir().unwrap();
        let pfad = ordner.path().join("diktat-1.json");
        std::fs::write(&pfad, "{}").unwrap();
        drop(Aufraeumer(pfad.clone()));
        assert!(!pfad.exists(), "das Transkript darf nicht liegen bleiben");
    }

    #[test]
    fn laufzeit_verzeichnis_liegt_unter_der_laufzeit() {
        let pfad = laufzeit_verzeichnis();
        assert!(pfad.ends_with("vergissmeinnicht"));
        // Aufnahmen gehören nicht ins Datenverzeichnis der Replica.
        assert_ne!(pfad, crate::config::data_dir());
    }

    // ─── Echte Läufe (nur auf Anforderung) ──────────────────────────────────
    //
    // Beide brauchen eine Maschine mit PipeWire; der Roundtrip zusätzlich ein
    // installiertes Backend. Deshalb `#[ignore]` — wie `cli_coexistence` im
    // Core:
    //   cargo test -p vergissmeinnicht-app -- --ignored diktat --nocapture

    /// Der Kern der Beendigungskette: Nach SIGINT hat `pw-record` die
    /// RIFF-Größenfelder nachgetragen. Ein hart abgeschossener Recorder
    /// hinterlässt dort den Platzhalter 8 — die Datei sieht gleich aus, ist
    /// für jedes Whisper-Backend aber leer.
    #[test]
    #[ignore]
    fn diktat_aufnahme_hat_gueltigen_wav_header() {
        let mut aufnahme = Aufnahme::starte().expect("pw-record muss startbar sein");
        let wav = aufnahme.wav().to_path_buf();
        std::thread::sleep(Duration::from_secs(2));
        aufnahme.stoppe().expect("SIGINT muss reichen");
        let daten = std::fs::read(&wav).expect("WAV muss lesbar sein");
        assert!(daten.len() > 44, "WAV muss Audiodaten tragen");
        let groesse = u32::from_le_bytes([daten[4], daten[5], daten[6], daten[7]]);
        assert_eq!(
            groesse as usize,
            daten.len() - 8,
            "RIFF-Größenfeld muss nachgetragen sein (8 = hart abgeschossen)"
        );
        drop(aufnahme);
        assert!(!wav.exists(), "die Aufnahme muss nach Gebrauch verschwinden");
    }

    /// Ganze Kette mit dem konfigurierten Backend. Nimmt zwei Sekunden auf —
    /// bei Stille meldet das Backend `Leer`, und auch das ist ein Erfolg:
    /// geprüft wird der Weg, nicht der Wortlaut.
    #[test]
    #[ignore]
    fn diktat_roundtrip_mit_echtem_backend() {
        // Das kleine Modell, damit der Lauf nichts nachlädt.
        let s = Settings { ai_whisper_model: "base".into(), ..Settings::default() };
        verfuegbarkeit(&s).expect("Diktat-Kette muss vollständig sein");
        let mut aufnahme = Aufnahme::starte().expect("pw-record muss startbar sein");
        let wav = aufnahme.wav().to_path_buf();
        std::thread::sleep(Duration::from_secs(2));
        aufnahme.stoppe().expect("SIGINT muss reichen");
        let ergebnis = transkribiere(&s, &wav);
        println!("Transkript: {ergebnis:?}");
        assert!(
            !wav.with_extension("json").exists(),
            "das Transkript darf nicht im Laufzeitverzeichnis liegen bleiben"
        );
        match ergebnis {
            Ok(_) | Err(TranskriptFehler::Leer) => {}
            Err(e) => panic!("unerwarteter Fehler: {e}"),
        }
        drop(aufnahme);
        assert!(!wav.exists(), "die Aufnahme muss nach Gebrauch verschwinden");
    }

    #[test]
    fn letzte_zeile_meldet_die_aussagekraeftige_zeile() {
        assert_eq!(letzte_zeile("Lade Modell\nerror: no such file\n\n"), "error: no such file");
        assert_eq!(letzte_zeile("   "), "keine Fehlerausgabe");
    }
}
