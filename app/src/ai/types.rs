//! Vorschlags-Datentypen (Spec §4.1) — hier: der validierte Entwurf der
//! NL-Erfassung (Stufe 1). Es erreichen nur geprüfte Werte die UI (Spec §7):
//! Ungültiges lässt das jeweilige Feld leer, statt Müll in den Dialog zu
//! schreiben. Die Bridge publiziert das Ergebnis als JSON in `aiDraftJson`.

use serde::Serialize;

/// Validierter Entwurf aus der Modellantwort `{title, project, tags, due,
/// priority, recur, notes}`. Alle Felder sind leer-fähig; QML füllt daraus
/// die Dialogfelder (Füllfunktion `applyDraft` im QuickCaptureDialog).
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct AiDraft {
    pub title: String,
    pub project: String,
    pub tags: Vec<String>,
    pub due: String,
    pub priority: String,
    pub recur: String,
    pub notes: String,
}

impl AiDraft {
    /// Zieht die Schema-Felder aus der (bereits als JSON-Objekt validierten)
    /// Modellantwort und prüft sie einzeln: `due` über
    /// [`crate::parsers::parse_due_date`], `recur` über
    /// [`crate::parsers::is_valid_recur`], `priority` gegen H/M/L.
    /// Fehlende, falsch typisierte oder ungültige Werte ergeben leere Felder.
    pub fn aus_antwort(wert: &serde_json::Value, now: i64) -> Self {
        let due = string_feld(wert, "due");
        let recur = string_feld(wert, "recur").to_lowercase();
        let priority = string_feld(wert, "priority").to_ascii_uppercase();
        Self {
            title: string_feld(wert, "title"),
            project: string_feld(wert, "project"),
            tags: tags_feld(wert),
            due: if crate::parsers::parse_due_date(&due, now).is_some() {
                due
            } else {
                String::new()
            },
            // Nur die Taskwarrior-Standardwerte — wie der QuickCapture-Parser.
            priority: if matches!(priority.as_str(), "H" | "M" | "L") {
                priority
            } else {
                String::new()
            },
            recur: if !recur.is_empty() && crate::parsers::is_valid_recur(&recur) {
                recur
            } else {
                String::new()
            },
            notes: string_feld(wert, "notes"),
        }
    }
}

/// String-Feld mit Trim; fehlend, `null` oder falsch typisiert → leer.
fn string_feld(wert: &serde_json::Value, feld: &str) -> String {
    wert.get(feld)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Tags-Feld: Array von Strings (Nicht-Strings werden übergangen) oder ein
/// einzelner String mit Leerzeichen-Trennung. Führende `+`/`#` werden
/// entfernt — Modelle hängen gern die Capture-Syntax an.
fn tags_feld(wert: &serde_json::Value) -> Vec<String> {
    let roh: Vec<String> = match wert.get("tags") {
        Some(serde_json::Value::Array(eintraege)) => eintraege
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_string)
            .collect(),
        Some(serde_json::Value::String(s)) => {
            s.split_whitespace().map(str::to_string).collect()
        }
        _ => Vec::new(),
    };
    roh.iter()
        .map(|t| t.trim().trim_start_matches(['+', '#']).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fester Bezugszeitpunkt für die due-Validierung (nur Gültigkeit zählt).
    const NOW: i64 = 1_800_000_000;

    #[test]
    fn uebernimmt_gueltige_werte() {
        let antwort = serde_json::json!({
            "title": " Zahnarzttermin vereinbaren ",
            "project": "Gesundheit",
            "tags": ["arzt", "+telefon"],
            "due": "tomorrow",
            "priority": "h",
            "recur": "QUARTERLY",
            "notes": "Vormittags anrufen",
        });
        let draft = AiDraft::aus_antwort(&antwort, NOW);
        assert_eq!(draft.title, "Zahnarzttermin vereinbaren");
        assert_eq!(draft.project, "Gesundheit");
        assert_eq!(draft.tags, vec!["arzt", "telefon"]);
        assert_eq!(draft.due, "tomorrow");
        // Groß-/Kleinschreibung wird normalisiert (H/M/L, recur klein).
        assert_eq!(draft.priority, "H");
        assert_eq!(draft.recur, "quarterly");
        assert_eq!(draft.notes, "Vormittags anrufen");
    }

    #[test]
    fn ungueltige_werte_lassen_felder_leer() {
        // AC AI-B1: due über parse_due_date, recur über is_valid_recur
        // geprüft; Ungültiges bleibt leer statt Müll ins Formular zu schreiben.
        let antwort = serde_json::json!({
            "title": "Kaputte Metadaten",
            "due": "übermorgen vielleicht",
            "priority": "urgent",
            "recur": "alle Jubeljahre",
        });
        let draft = AiDraft::aus_antwort(&antwort, NOW);
        assert_eq!(draft.title, "Kaputte Metadaten");
        assert_eq!(draft.due, "");
        assert_eq!(draft.priority, "");
        assert_eq!(draft.recur, "");
    }

    #[test]
    fn gueltige_due_formen_bleiben_erhalten() {
        for token in ["today", "+3d", "2027-01-15", "friday", "eow"] {
            let antwort = serde_json::json!({"due": token});
            assert_eq!(AiDraft::aus_antwort(&antwort, NOW).due, token, "{token}");
        }
    }

    #[test]
    fn fehlende_und_falsch_typisierte_felder_sind_leer() {
        // Modelle liefern gern null oder Zahlen, wo Strings erwartet werden.
        let antwort = serde_json::json!({
            "title": null,
            "project": 7,
            "tags": [1, "echt", null],
            "due": 20260805,
        });
        let draft = AiDraft::aus_antwort(&antwort, NOW);
        assert_eq!(draft.title, "");
        assert_eq!(draft.project, "");
        // Nicht-String-Einträge werden übergangen, gültige bleiben.
        assert_eq!(draft.tags, vec!["echt"]);
        assert_eq!(draft.due, "");
        assert_eq!(draft.notes, "");
    }

    #[test]
    fn tags_als_string_werden_gesplittet() {
        let antwort = serde_json::json!({"tags": "arzt  #telefon +dringend"});
        assert_eq!(
            AiDraft::aus_antwort(&antwort, NOW).tags,
            vec!["arzt", "telefon", "dringend"]
        );
    }

    #[test]
    fn serialisiert_alle_schema_felder() {
        // QML verlässt sich auf vorhandene Schlüssel (auch leere).
        let json = serde_json::to_value(AiDraft::default()).unwrap();
        for feld in ["title", "project", "tags", "due", "priority", "recur", "notes"] {
            assert!(json.get(feld).is_some(), "Feld {feld} fehlt im JSON");
        }
    }
}
