//! Prompt-Bausteine je Feature (Spec §4.1) — Qt-frei. Jeder Prompt, der
//! Datumsangaben auflöst, bekommt das aktuelle Datum samt Uhrzeit; jeder, der
//! Projekte/Tags zuordnet, die vorhandene Taxonomie — so mappt das Modell in
//! bestehende Kategorien, statt neue zu erfinden (neue Projektnamen bleiben
//! ausdrücklich erlaubt, Spec §5 Stufe 1).
//!
//! Reihenfolge im System-Prompt (AI-B1b, #31): **Stabiles zuerst, Flüchtiges
//! zuletzt.** Schema, Regeln, Taxonomie und Aufgabenliste ändern sich selten;
//! die Datumszeile ändert sich jede Minute. Stünde sie vorn, würde sie den
//! Präfix-Cache des Backends bei fast jeder Anfrage entwerten (gemessen bei
//! Stufe C: 21,4 s statt 0,11 s Prompt-Verarbeitung).

use super::client::ChatMessage;
use vergissmeinnicht_core::chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Weekday};
use vergissmeinnicht_core::{TaskInfo, TaskStatus};

/// Antwortschema der NL-Erfassung (Stufe 1) — wortgleich im Prompt und in der
/// Validierung ([`super::types::AiDraft`]).
const CAPTURE_SCHEMA: &str = r#"{"title": string, "project": string, "tags": [string], "due": string, "priority": string, "recur": string, "notes": string}"#;

/// Zeichen-Budget für den Aufgabenblock (AI-B1b, #31). Bei den gemessenen
/// ~3,2 Zeichen pro Token entspricht das rund 31k Tokens — genug für weit
/// über 1000 Aufgaben, aber sicher unter dem 64k-Kontextfenster des
/// Referenz-Backends (Stufe C wächst sonst unbegrenzt mit den Erledigten).
const AUFGABEN_BUDGET_ZEICHEN: usize = 100_000;

/// Kontextumfang der KI-Interpretation (AI-B1b, #31) — was der Prompt über
/// den Aufgabenbestand verrät. Gelöschte Aufgaben sind auf keiner Stufe
/// enthalten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kontextstufe {
    /// Stufe A (Default, bisheriges Verhalten): nur Projekt- und
    /// Schlagwortnamen.
    Taxonomie,
    /// Stufe B: zusätzlich die Titel aller offenen (pending) Aufgaben.
    OffeneTitel,
    /// Stufe C: alle nicht gelöschten Aufgaben kompakt (Titel, Projekt,
    /// Schlagworte, Fälligkeit, Status-Markierung).
    Alle,
}

impl Kontextstufe {
    /// Config-Wert (`ai_context_level`) → Stufe. Unbekanntes fällt auf die
    /// datensparsamste Stufe zurück.
    pub fn aus_config(wert: &str) -> Self {
        match wert {
            "open_titles" => Self::OffeneTitel,
            "all" => Self::Alle,
            _ => Self::Taxonomie,
        }
    }
}

/// Baut die Nachrichten für „Mit KI interpretieren" (Story AI-B1): System-
/// Nachricht mit Schema, Regeln, Taxonomie, Aufgabenliste je Kontextstufe
/// (AI-B1b) und aktuellem Datum; die rohe Eingabe als User-Nachricht. Die
/// JSON-Erzwingung ergänzt `complete_json`.
pub fn capture_nachrichten(
    eingabe: &str,
    jetzt: DateTime<Local>,
    projekte: &[String],
    tags: &[String],
    stufe: Kontextstufe,
    aufgaben: &[TaskInfo],
) -> Vec<ChatMessage> {
    // Stabiler Teil: Schema, Regeln, Taxonomie — ändert sich nur, wenn der
    // Nutzer seine Daten ändert (Präfix-Cache-freundlich, siehe Modul-Doku).
    let mut system = format!(
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
         Vorhandene Projekte: {projekte}.\n\
         Vorhandene Schlagworte: {schlagworte}.",
        projekte = liste_oder_keine(projekte),
        schlagworte = liste_oder_keine(tags),
    );
    if let Some(block) = aufgaben_block(stufe, aufgaben, AUFGABEN_BUDGET_ZEICHEN) {
        system.push('\n');
        system.push_str(&block);
    }
    // Flüchtiger Teil zuletzt: die Minutenangabe ändert sich bei jeder
    // Anfrage — hinter der Aufgabenliste bleibt der lange Präfix cachebar.
    system.push_str(&format!(
        "\nAktuelles Datum: {wochentag}, {datum}, {uhrzeit} Uhr.",
        wochentag = wochentag_deutsch(jetzt.weekday()),
        datum = jetzt.format("%Y-%m-%d"),
        uhrzeit = format_args!("{:02}:{:02}", jetzt.hour(), jetzt.minute()),
    ));
    vec![ChatMessage::system(&system), ChatMessage::user(eingabe)]
}

/// Aufgabenblock je Kontextstufe; `None` für Stufe A. Gelöschte Aufgaben
/// werden defensiv herausgefiltert, obwohl `state.tasks` sie nie enthält —
/// die Zusicherung „Gelöschtes verlässt nie die Maschine" hängt nicht an
/// einem Aufrufer. Das `budget` (Zeichen) deckelt Stufe C: Offene bleiben
/// immer vollständig, Erledigte fliegen älteste zuerst, und die Kürzung
/// wird im Prompt benannt.
fn aufgaben_block(stufe: Kontextstufe, aufgaben: &[TaskInfo], budget: usize) -> Option<String> {
    match stufe {
        Kontextstufe::Taxonomie => None,
        Kontextstufe::OffeneTitel => {
            let zeilen: Vec<String> = aufgaben
                .iter()
                .filter(|t| t.status == TaskStatus::Pending)
                .map(|t| format!("- {}", t.description))
                .collect();
            let mut block = "Offene Aufgaben (Titel):".to_string();
            if zeilen.is_empty() {
                block.push_str("\n(keine)");
            } else {
                for z in &zeilen {
                    block.push('\n');
                    block.push_str(z);
                }
            }
            Some(block)
        }
        Kontextstufe::Alle => {
            let sichtbar: Vec<&TaskInfo> = aufgaben
                .iter()
                .filter(|t| t.status != TaskStatus::Deleted)
                .collect();
            // Erledigte nach Alter (jüngste zuerst) ins Budget einsortieren;
            // Offene und Recurring-Master zählen vorab voll ins Budget und
            // werden nie weggelassen. Als Alters-Proxy dient `modified`
            // (letzte Änderung ≈ Erledigungszeitpunkt), ersatzweise `entry`.
            let zeilen: Vec<String> = sichtbar.iter().map(|t| kompakt_zeile(t)).collect();
            let mut verbraucht: usize = sichtbar
                .iter()
                .zip(&zeilen)
                .filter(|(t, _)| t.status != TaskStatus::Completed)
                .map(|(_, z)| z.len() + 1)
                .sum();
            let mut erledigte: Vec<(usize, i64)> = sichtbar
                .iter()
                .enumerate()
                .filter(|(_, t)| t.status == TaskStatus::Completed)
                .map(|(i, t)| (i, t.modified.or(t.entry).unwrap_or(0)))
                .collect();
            erledigte.sort_by_key(|(_, zeit)| std::cmp::Reverse(*zeit));
            let mut behalten = vec![true; sichtbar.len()];
            let mut gekuerzt = false;
            for (i, _) in erledigte {
                let kosten = zeilen[i].len() + 1;
                if !gekuerzt && verbraucht + kosten <= budget {
                    verbraucht += kosten;
                } else {
                    // Ab der ersten zu alten Erledigten fliegt der Rest mit —
                    // so bleibt „älteste zuerst weglassen" strikt erfüllt.
                    gekuerzt = true;
                    behalten[i] = false;
                }
            }
            let mut block = "Alle Aufgaben (kompakt):".to_string();
            let mut leer = true;
            for (i, z) in zeilen.iter().enumerate() {
                if behalten[i] {
                    block.push('\n');
                    block.push_str(z);
                    leer = false;
                }
            }
            if leer {
                block.push_str("\n(keine)");
            }
            if gekuerzt {
                block.push_str(
                    "\nHinweis: Liste aus Platzgründen gekürzt — die ältesten \
                     erledigten Aufgaben wurden weggelassen.",
                );
            }
            Some(block)
        }
    }
}

/// Kompakte Einzeiler-Darstellung für Stufe C: Status-Markierung, Titel und
/// nur die tatsächlich gesetzten Metadaten.
fn kompakt_zeile(t: &TaskInfo) -> String {
    let marker = match t.status {
        TaskStatus::Completed => "erledigt",
        TaskStatus::Recurring => "wiederkehrend",
        _ => "offen",
    };
    let mut teile: Vec<String> = Vec::new();
    if let Some(p) = &t.project {
        teile.push(format!("Projekt: {p}"));
    }
    if !t.tags.is_empty() {
        teile.push(format!("Schlagworte: {}", t.tags.join(", ")));
    }
    if let Some(due) = t.due {
        if let Some(datum) = Local.timestamp_opt(due, 0).single() {
            teile.push(format!("fällig: {}", datum.format("%Y-%m-%d")));
        }
    }
    if teile.is_empty() {
        format!("- [{marker}] {}", t.description)
    } else {
        format!("- [{marker}] {} ({})", t.description, teile.join("; "))
    }
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

    fn jetzt_fest() -> DateTime<Local> {
        // Mittwoch, 5. August 2026, 21:40 lokale Zeit.
        Local.with_ymd_and_hms(2026, 8, 5, 21, 40, 0).unwrap()
    }

    fn aufgabe(titel: &str, status: TaskStatus) -> TaskInfo {
        TaskInfo {
            uuid: titel.into(),
            description: titel.into(),
            project: None,
            tags: vec![],
            due: None,
            status,
            entry: None,
            working_set_id: None,
            priority: None,
            annotations: vec![],
            wait: None,
            recur: None,
            scheduled: None,
            depends: vec![],
            is_blocked: false,
            is_blocking: false,
            is_recurring_child: false,
            start: None,
            until: None,
            modified: None,
            udas: vec![],
        }
    }

    /// Gemischter Bestand für die Stufen-Tests: offen, erledigt, gelöscht,
    /// Recurring-Master.
    fn bestand() -> Vec<TaskInfo> {
        vec![
            aufgabe("Offene Aufgabe Eins", TaskStatus::Pending),
            aufgabe("Erledigte Aufgabe Zwei", TaskStatus::Completed),
            aufgabe("Gelöschte Aufgabe Drei", TaskStatus::Deleted),
            aufgabe("Wiederkehrende Aufgabe Vier", TaskStatus::Recurring),
        ]
    }

    fn system_prompt(stufe: Kontextstufe, aufgaben: &[TaskInfo]) -> String {
        capture_nachrichten("x", jetzt_fest(), &[], &[], stufe, aufgaben)[0]
            .content
            .clone()
    }

    #[test]
    fn capture_prompt_traegt_schema_datum_und_eingabe() {
        let nachrichten = capture_nachrichten(
            "Zahnarzt nächste Woche Dienstag, wichtig",
            jetzt_fest(),
            &[],
            &[],
            Kontextstufe::Taxonomie,
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
        let nachrichten = capture_nachrichten(
            "x",
            jetzt_fest(),
            &projekte,
            &tags,
            Kontextstufe::Taxonomie,
            &[],
        );
        let system = &nachrichten[0].content;
        assert!(system.contains("Vorhandene Projekte: Büro, Garten.Beet."));
        assert!(system.contains("Vorhandene Schlagworte: dringend, arzt."));
        // Neue Projektnamen sind laut Prompt ausdrücklich erlaubt (AC AI-B1).
        assert!(system.contains("neuer Name ist erlaubt"));
    }

    #[test]
    fn capture_prompt_benennt_leere_taxonomie() {
        let nachrichten =
            capture_nachrichten("x", jetzt_fest(), &[], &[], Kontextstufe::Taxonomie, &[]);
        assert!(nachrichten[0].content.contains("Vorhandene Projekte: (keine)."));
        assert!(nachrichten[0].content.contains("Vorhandene Schlagworte: (keine)."));
    }

    #[test]
    fn stufe_aus_config_mit_rueckfall() {
        assert_eq!(Kontextstufe::aus_config("taxonomy"), Kontextstufe::Taxonomie);
        assert_eq!(Kontextstufe::aus_config("open_titles"), Kontextstufe::OffeneTitel);
        assert_eq!(Kontextstufe::aus_config("all"), Kontextstufe::Alle);
        // Unbekanntes (alte/kaputte Config) fällt auf die datensparsamste Stufe.
        assert_eq!(Kontextstufe::aus_config("unbekannt"), Kontextstufe::Taxonomie);
        assert_eq!(Kontextstufe::aus_config(""), Kontextstufe::Taxonomie);
    }

    #[test]
    fn stufe_taxonomie_traegt_keine_aufgabentitel() {
        let system = system_prompt(Kontextstufe::Taxonomie, &bestand());
        assert!(!system.contains("Offene Aufgabe Eins"));
        assert!(!system.contains("Erledigte Aufgabe Zwei"));
        assert!(!system.contains("Offene Aufgaben"));
        assert!(!system.contains("Alle Aufgaben"));
    }

    #[test]
    fn stufe_offene_titel_traegt_nur_pending_titel() {
        let system = system_prompt(Kontextstufe::OffeneTitel, &bestand());
        assert!(system.contains("Offene Aufgaben (Titel):"));
        assert!(system.contains("- Offene Aufgabe Eins"));
        // Erledigte, Gelöschte und Recurring-Master gehören nicht in Stufe B.
        assert!(!system.contains("Erledigte Aufgabe Zwei"));
        assert!(!system.contains("Gelöschte Aufgabe Drei"));
        assert!(!system.contains("Wiederkehrende Aufgabe Vier"));
    }

    #[test]
    fn stufe_alle_traegt_alles_ausser_geloeschtem_kompakt() {
        let mut erledigt = aufgabe("Erledigte Aufgabe Zwei", TaskStatus::Completed);
        erledigt.project = Some("Garten".into());
        erledigt.tags = vec!["beet".into()];
        // 2026-08-01 12:00 lokale Zeit als Fälligkeit.
        erledigt.due =
            Some(Local.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap().timestamp());
        let aufgaben = vec![
            aufgabe("Offene Aufgabe Eins", TaskStatus::Pending),
            erledigt,
            aufgabe("Gelöschte Aufgabe Drei", TaskStatus::Deleted),
            aufgabe("Wiederkehrende Aufgabe Vier", TaskStatus::Recurring),
        ];
        let system = system_prompt(Kontextstufe::Alle, &aufgaben);
        assert!(system.contains("Alle Aufgaben (kompakt):"));
        assert!(system.contains("- [offen] Offene Aufgabe Eins"));
        assert!(system.contains(
            "- [erledigt] Erledigte Aufgabe Zwei (Projekt: Garten; Schlagworte: beet; fällig: 2026-08-01)"
        ));
        assert!(system.contains("- [wiederkehrend] Wiederkehrende Aufgabe Vier"));
        assert!(!system.contains("Gelöschte Aufgabe Drei"));
    }

    #[test]
    fn geloeschte_aufgaben_erreichen_keine_stufe() {
        // Defensiv-Zusicherung aus #31: Gelöschtes verlässt nie die Maschine —
        // auf keiner Stufe, selbst wenn ein Aufrufer sie hereinreicht.
        for stufe in [Kontextstufe::Taxonomie, Kontextstufe::OffeneTitel, Kontextstufe::Alle] {
            let system = system_prompt(stufe, &bestand());
            assert!(
                !system.contains("Gelöschte Aufgabe Drei"),
                "Gelöschtes in Stufe {stufe:?}"
            );
        }
    }

    #[test]
    fn datum_steht_nach_der_aufgabenliste() {
        // Befund 1 aus #31: Flüchtiges (Datum mit Minute) muss HINTER dem
        // stabilen Teil stehen, sonst entwertet jede Anfrage den
        // Präfix-Cache des Backends.
        for stufe in [Kontextstufe::Taxonomie, Kontextstufe::OffeneTitel, Kontextstufe::Alle] {
            let system = system_prompt(stufe, &bestand());
            let datum = system.find("Aktuelles Datum:").expect("Datumszeile fehlt");
            let taxonomie = system.find("Vorhandene Schlagworte:").unwrap();
            assert!(datum > taxonomie, "Datum vor der Taxonomie (Stufe {stufe:?})");
            if let Some(liste) = system.find("Aufgaben") {
                assert!(datum > liste, "Datum vor der Aufgabenliste (Stufe {stufe:?})");
            }
            // Nach der Datumszeile kommt nichts mehr.
            assert!(system.trim_end().ends_with("Uhr."), "Datum nicht am Ende");
        }
    }

    #[test]
    fn deckel_wirft_aelteste_erledigte_und_behaelt_offene() {
        // Befund 2 aus #31: Budget-Deckel — Offene bleiben immer, Erledigte
        // fliegen älteste zuerst, die Kürzung wird benannt.
        let mut alt = aufgabe("Uralte erledigte Aufgabe", TaskStatus::Completed);
        alt.modified = Some(1_000);
        let mut mittel = aufgabe("Mittlere erledigte Aufgabe", TaskStatus::Completed);
        mittel.modified = Some(2_000);
        let mut neu = aufgabe("Frische erledigte Aufgabe", TaskStatus::Completed);
        neu.modified = Some(3_000);
        let aufgaben = vec![
            aufgabe("Offene Aufgabe bleibt", TaskStatus::Pending),
            alt,
            mittel,
            neu,
        ];
        // Budget reicht für die Offene und genau eine Erledigte.
        let block = aufgaben_block(Kontextstufe::Alle, &aufgaben, 80).unwrap();
        assert!(block.contains("Offene Aufgabe bleibt"));
        assert!(block.contains("Frische erledigte Aufgabe"));
        assert!(!block.contains("Mittlere erledigte Aufgabe"));
        assert!(!block.contains("Uralte erledigte Aufgabe"));
        assert!(block.contains("gekürzt"));

        // Ohne Budgetdruck: alles drin, kein Kürzungshinweis.
        let voll = aufgaben_block(Kontextstufe::Alle, &aufgaben, 10_000).unwrap();
        assert!(voll.contains("Uralte erledigte Aufgabe"));
        assert!(!voll.contains("gekürzt"));

        // Offene überleben selbst ein absurd kleines Budget.
        let winzig = aufgaben_block(Kontextstufe::Alle, &aufgaben, 1).unwrap();
        assert!(winzig.contains("Offene Aufgabe bleibt"));
        assert!(!winzig.contains("erledigte Aufgabe"));
        assert!(winzig.contains("gekürzt"));
    }
}
