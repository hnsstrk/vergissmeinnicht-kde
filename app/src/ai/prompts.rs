//! Prompt-Bausteine je Feature (Spec §4.1) — Qt-frei. Jeder Prompt, der
//! Datumsangaben auflöst, bekommt das aktuelle Datum samt Uhrzeit; jeder, der
//! Projekte/Tags zuordnet, die vorhandene Taxonomie — so mappt das Modell in
//! bestehende Kategorien, statt neue zu erfinden (neue Projektnamen bleiben
//! ausdrücklich erlaubt, Spec §5 Stufe 1).

use super::client::ChatMessage;
use vergissmeinnicht_core::chrono::{DateTime, Datelike, Local, Timelike, Weekday};

/// Antwortschema der NL-Erfassung (Stufe 1) — wortgleich im Prompt und in der
/// Validierung ([`super::types::AiDraft`]).
const CAPTURE_SCHEMA: &str = r#"{"title": string, "project": string, "tags": [string], "due": string, "priority": string, "recur": string, "notes": string}"#;

/// Baut die Nachrichten für „Mit KI interpretieren" (Story AI-B1): System-
/// Nachricht mit Schema, Regeln, aktuellem Datum und Taxonomie; die rohe
/// Eingabe als User-Nachricht. Die JSON-Erzwingung ergänzt `complete_json`.
pub fn capture_nachrichten(
    eingabe: &str,
    jetzt: DateTime<Local>,
    projekte: &[String],
    tags: &[String],
) -> Vec<ChatMessage> {
    let system = format!(
        "Du wandelst eine natürlichsprachige Aufgabenbeschreibung in eine \
         strukturierte Taskwarrior-Aufgabe um. Antworte mit genau einem \
         JSON-Objekt der Form {CAPTURE_SCHEMA}.\n\
         Regeln:\n\
         - title: prägnanter Aufgabentitel ohne Datums- und Metadaten-Floskeln.\n\
         - project: passendes Projekt — bevorzugt eines der vorhandenen; ein \
         neuer Name ist erlaubt, wenn keines passt; sonst leer.\n\
         - tags: passende Schlagworte (bevorzugt vorhandene), sonst leeres Array.\n\
         - due: Fälligkeit als Taskwarrior-Ausdruck: today, tomorrow, +Nd, +Nw, \
         ein englischer Wochentag (z. B. friday) oder ein ISO-Datum \
         (JJJJ-MM-TT); sonst leer.\n\
         - priority: \"H\", \"M\" oder \"L\"; sonst leer.\n\
         - recur: Wiederholung als daily, weekly, monthly, quarterly, yearly \
         oder Intervall wie 3d, 2w, 6m, 1y; sonst leer.\n\
         - notes: ergänzende Details, die nicht in den Titel gehören; sonst leer.\n\
         Setze nur, was die Eingabe wirklich hergibt — rate nicht.\n\
         Aktuelles Datum: {wochentag}, {datum}, {uhrzeit} Uhr.\n\
         Vorhandene Projekte: {projekte}.\n\
         Vorhandene Schlagworte: {schlagworte}.",
        wochentag = wochentag_deutsch(jetzt.weekday()),
        datum = jetzt.format("%Y-%m-%d"),
        uhrzeit = format_args!("{:02}:{:02}", jetzt.hour(), jetzt.minute()),
        projekte = liste_oder_keine(projekte),
        schlagworte = liste_oder_keine(tags),
    );
    vec![ChatMessage::system(&system), ChatMessage::user(eingabe)]
}

/// Kommagetrennte Liste; leere Taxonomie wird ausdrücklich benannt, damit das
/// Modell nicht über fehlende Angaben spekuliert.
fn liste_oder_keine(namen: &[String]) -> String {
    if namen.is_empty() {
        "(keine)".to_string()
    } else {
        namen.join(", ")
    }
}

/// Deutscher Wochentagsname — chrono formatiert `%A` nur englisch.
fn wochentag_deutsch(tag: Weekday) -> &'static str {
    match tag {
        Weekday::Mon => "Montag",
        Weekday::Tue => "Dienstag",
        Weekday::Wed => "Mittwoch",
        Weekday::Thu => "Donnerstag",
        Weekday::Fri => "Freitag",
        Weekday::Sat => "Samstag",
        Weekday::Sun => "Sonntag",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vergissmeinnicht_core::chrono::TimeZone;

    fn jetzt_fest() -> DateTime<Local> {
        // Mittwoch, 5. August 2026, 21:40 lokale Zeit.
        Local.with_ymd_and_hms(2026, 8, 5, 21, 40, 0).unwrap()
    }

    #[test]
    fn capture_prompt_traegt_schema_datum_und_eingabe() {
        let nachrichten = capture_nachrichten(
            "Zahnarzt nächste Woche Dienstag, wichtig",
            jetzt_fest(),
            &[],
            &[],
        );
        assert_eq!(nachrichten.len(), 2);
        assert_eq!(nachrichten[0].role, "system");
        // Schema-Felder und Datumskontext stehen in der System-Nachricht.
        for feld in ["title", "project", "tags", "due", "priority", "recur", "notes"] {
            assert!(nachrichten[0].content.contains(feld), "Schema-Feld {feld} fehlt");
        }
        assert!(nachrichten[0].content.contains("Mittwoch, 2026-08-05, 21:40 Uhr"));
        // Die rohe Eingabe ist die User-Nachricht — unverändert.
        assert_eq!(nachrichten[1].role, "user");
        assert_eq!(nachrichten[1].content, "Zahnarzt nächste Woche Dienstag, wichtig");
    }

    #[test]
    fn capture_prompt_traegt_taxonomie_und_erlaubt_neue_projekte() {
        let projekte = vec!["Büro".to_string(), "Garten.Beet".to_string()];
        let tags = vec!["dringend".to_string(), "arzt".to_string()];
        let nachrichten = capture_nachrichten("x", jetzt_fest(), &projekte, &tags);
        let system = &nachrichten[0].content;
        assert!(system.contains("Vorhandene Projekte: Büro, Garten.Beet."));
        assert!(system.contains("Vorhandene Schlagworte: dringend, arzt."));
        // Neue Projektnamen sind laut Prompt ausdrücklich erlaubt (AC AI-B1).
        assert!(system.contains("neuer Name ist erlaubt"));
    }

    #[test]
    fn capture_prompt_benennt_leere_taxonomie() {
        let nachrichten = capture_nachrichten("x", jetzt_fest(), &[], &[]);
        assert!(nachrichten[0].content.contains("Vorhandene Projekte: (keine)."));
        assert!(nachrichten[0].content.contains("Vorhandene Schlagworte: (keine)."));
    }
}
