//! Parser-Ports aus der macOS-Version: QuickCaptureParser, DueDateParser, RecurParser.
//! Verhalten ist 1:1 übernommen, damit Eingaben auf beiden Plattformen gleich wirken.

use vergissmeinnicht_core::chrono::{Datelike, Duration, Local, Months, NaiveDate, TimeZone, Weekday};

// ─── QuickCaptureParser ─────────────────────────────────────────────────────

/// Geparster Vorschau-Snapshot einer QuickCapture-Eingabe.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct QuickCapturePreview {
    pub description: String,
    pub tags: Vec<String>,
    pub project: Option<String>,
    pub due: Option<String>,
    pub priority: Option<String>,
}

/// Parser für Taskwarrior-ähnliche QuickCapture-Eingaben.
///
/// Erkannt werden:
/// - `+tag` → in `tags` (ohne `+`)
/// - `project:value` → `project`
/// - `due:value` → `due`
/// - `priority:value` → `priority`
/// - `\ ` (Backslash + Leerzeichen) innerhalb eines Tokens als literales Leerzeichen
///   in der Description (z.B. `meeting\ notes +work` → description `"meeting notes"`)
///
/// Alle übrigen Tokens bilden in der Eingabe-Reihenfolge die `description`.
pub fn parse_quick_capture(input: &str) -> QuickCapturePreview {
    let mut description_tokens: Vec<String> = Vec::new();
    let mut preview = QuickCapturePreview::default();

    for token in tokenize_escaped(input) {
        if let Some(value) = strip_prefix_nonempty("project:", &token) {
            preview.project = Some(value.to_string());
        } else if let Some(value) = strip_prefix_nonempty("due:", &token) {
            preview.due = Some(value.to_string());
        } else if let Some(value) = strip_prefix_nonempty("priority:", &token) {
            // Nur die Taskwarrior-Standardwerte H/M/L (case-insensitiv) — andere
            // Werte bleiben Beschreibungstext, statt stumm eine für die CLI
            // unbekannte Priorität zu schreiben.
            let normalized = value.to_ascii_uppercase();
            if matches!(normalized.as_str(), "H" | "M" | "L") {
                preview.priority = Some(normalized);
            } else {
                description_tokens.push(token);
            }
        } else if let Some(tag) = token.strip_prefix('+').filter(|t| !t.is_empty()) {
            preview.tags.push(tag.to_string());
        } else {
            description_tokens.push(token);
        }
    }

    preview.description = description_tokens.join(" ");
    preview
}

/// Splittet die Eingabe an Whitespace, behandelt aber `\ ` (Backslash gefolgt von
/// Leerzeichen) als literales Leerzeichen innerhalb des aktuellen Tokens. Andere
/// Backslash-Sequenzen werden unverändert übernommen.
fn tokenize_escaped(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(' ') => current.push(' '),
                Some(next) => {
                    current.push(ch);
                    current.push(next);
                }
                None => current.push(ch),
            }
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn strip_prefix_nonempty<'a>(prefix: &str, token: &'a str) -> Option<&'a str> {
    token.strip_prefix(prefix).filter(|rest| !rest.is_empty())
}

// ─── DueDateParser ──────────────────────────────────────────────────────────

/// Wandelt einen `due:`-Token in einen Unix-Sekunden-Timestamp.
///
/// Unterstützte Formen (klein-/großschreibungsunabhängig):
/// - `today`, `tomorrow` (plus deutsche Aliase `heute`, `morgen`)
/// - `+Nd` / `+Nw` (relativ: Tage / Wochen ab jetzt)
/// - `yyyy-MM-dd` (ISO-Datum)
///
/// Alle anderen Eingaben liefern `None`. Der Stichtag wird auf das **Ende des
/// Zieltages** in der lokalen Zeitzone gesetzt (23:59:59), damit "heute fällig"
/// nicht direkt nach Mitternacht in "überfällig" umkippt.
pub fn parse_due_date(value: &str, now: i64) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_lowercase();
    let today = local_date(now)?;

    if normalized == "today" || normalized == "heute" {
        return end_of_day(today);
    }
    if normalized == "tomorrow" || normalized == "morgen" {
        return end_of_day(today.succ_opt()?);
    }
    if let Some(rest) = normalized.strip_prefix('+') {
        if rest.len() >= 2 {
            let unit = rest.chars().last()?;
            let num_part = &rest[..rest.len() - 1];
            if let Ok(n) = num_part.parse::<i64>() {
                let target = match unit {
                    'd' => today.checked_add_signed(Duration::days(n))?,
                    'w' => today.checked_add_signed(Duration::weeks(n))?,
                    _ => return None,
                };
                return end_of_day(target);
            }
        }
        return None;
    }

    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return end_of_day(date);
    }
    None
}

fn local_date(now: i64) -> Option<NaiveDate> {
    Some(Local.timestamp_opt(now, 0).single()?.date_naive())
}

/// Letzte Sekunde des übergebenen Tages (23:59:59 Ortszeit) als Unix-Sekunden.
pub fn end_of_day(date: NaiveDate) -> Option<i64> {
    let next = date.succ_opt()?;
    let midnight = next.and_hms_opt(0, 0, 0)?;
    let local = Local
        .from_local_datetime(&midnight)
        .single()
        .or_else(|| Local.from_local_datetime(&midnight).earliest())?;
    Some(local.timestamp() - 1)
}

// ─── RecurParser ────────────────────────────────────────────────────────────

/// Wandelt einen Recur-Property-String (Taskwarrior-Format) in die nächste
/// Fälligkeit, ausgehend vom alten Due-Datum (Generator-Light).
///
/// Erkannte Formen:
/// - `daily`, `weekly`, `monthly`, `yearly`
/// - `Nd`, `Nw`, `Nm`, `Ny` (z.B. `3d`, `2w`)
///
/// Alles andere liefert `None` — die App erzeugt dann keine Folge-Instanz.
pub fn next_due_after(recur: &str, due: i64) -> Option<i64> {
    let trimmed = recur.trim().to_lowercase();
    if trimmed.is_empty() {
        return None;
    }

    // Taskwarrior-Frequenz-Synonyme (man task, „recur:"), plus Suffix-Formen
    // wie 3wks/2mo/1qtr. Einheit 'q' = Quartal, 'b' = nächster Werktag.
    let (n, unit): (u32, char) = match trimmed.as_str() {
        "daily" | "day" => (1, 'd'),
        "weekdays" => (1, 'b'),
        "weekly" => (1, 'w'),
        "biweekly" | "fortnight" => (2, 'w'),
        "monthly" | "month" => (1, 'm'),
        "quarterly" => (1, 'q'),
        "semiannual" => (6, 'm'),
        "annual" | "yearly" => (1, 'y'),
        "biannual" | "biyearly" => (2, 'y'),
        other => {
            let digits_end = other.find(|c: char| !c.is_ascii_digit())?;
            let n: u32 = other[..digits_end].parse().ok()?;
            if n == 0 {
                return None;
            }
            let unit = match &other[digits_end..] {
                "d" | "day" | "days" => 'd',
                "w" | "wk" | "wks" | "week" | "weeks" => 'w',
                "m" | "mo" | "mos" | "month" | "months" => 'm',
                "q" | "qtr" | "qtrs" | "quarter" | "quarters" => 'q',
                "y" | "yr" | "yrs" | "year" | "years" => 'y',
                _ => return None,
            };
            (n, unit)
        }
    };

    let local = Local.timestamp_opt(due, 0).single()?;
    let shifted = match unit {
        'd' => local.checked_add_signed(Duration::days(n as i64))?,
        'b' => {
            // Nächster Werktag (Mo–Fr) nach dem Fälligkeitsdatum.
            let mut next = local.checked_add_signed(Duration::days(1))?;
            while matches!(next.weekday(), Weekday::Sat | Weekday::Sun) {
                next = next.checked_add_signed(Duration::days(1))?;
            }
            next
        }
        'w' => local.checked_add_signed(Duration::weeks(n as i64))?,
        'm' => local.checked_add_months(Months::new(n))?,
        'q' => local.checked_add_months(Months::new(3 * n))?,
        'y' => local.with_year(local.year() + n as i32).or_else(|| {
            // 29. Februar + 1 Jahr → chrono liefert None; auf 28.2. ausweichen.
            local
                .with_day(28)
                .and_then(|d| d.with_year(local.year() + n as i32))
        })?,
        _ => return None,
    };
    Some(shifted.timestamp())
}

/// Prüft, ob ein Recur-String von `next_due_after` interpretiert werden kann.
pub fn is_valid_recur(recur: &str) -> bool {
    // Referenz-Zeitpunkt beliebig — es geht nur um die Syntax.
    next_due_after(recur, 1_800_000_000).is_some()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── QuickCapture ────────────────────────────────────────────────────────

    #[test]
    fn quick_capture_plain_description() {
        let p = parse_quick_capture("Milch kaufen");
        assert_eq!(p.description, "Milch kaufen");
        assert!(p.tags.is_empty());
        assert_eq!(p.project, None);
    }

    #[test]
    fn quick_capture_full_tokens() {
        let p = parse_quick_capture("Bericht schreiben +arbeit +wichtig project:Büro due:tomorrow priority:H");
        assert_eq!(p.description, "Bericht schreiben");
        assert_eq!(p.tags, vec!["arbeit", "wichtig"]);
        assert_eq!(p.project.as_deref(), Some("Büro"));
        assert_eq!(p.due.as_deref(), Some("tomorrow"));
        assert_eq!(p.priority.as_deref(), Some("H"));
    }

    #[test]
    fn quick_capture_priority_normalized_and_validated() {
        // Kleinschreibung wird normalisiert …
        let p = parse_quick_capture("Aufgabe priority:m");
        assert_eq!(p.priority.as_deref(), Some("M"));
        // … unbekannte Werte bleiben Beschreibungstext.
        let p = parse_quick_capture("Aufgabe priority:X");
        assert_eq!(p.priority, None);
        assert_eq!(p.description, "Aufgabe priority:X");
    }

    #[test]
    fn quick_capture_escaped_space() {
        let p = parse_quick_capture("meeting\\ notes +work");
        assert_eq!(p.description, "meeting notes");
        assert_eq!(p.tags, vec!["work"]);
    }

    #[test]
    fn quick_capture_bare_plus_is_description() {
        let p = parse_quick_capture("2 + 2 rechnen");
        assert_eq!(p.description, "2 + 2 rechnen");
        assert!(p.tags.is_empty());
    }

    #[test]
    fn quick_capture_empty_operator_values_stay_description() {
        let p = parse_quick_capture("project: due: priority: text");
        assert_eq!(p.description, "project: due: priority: text");
        assert_eq!(p.project, None);
    }

    // ── DueDateParser ───────────────────────────────────────────────────────

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn due_today_is_end_of_day() {
        let ts = parse_due_date("today", NOW).unwrap();
        assert!(ts >= NOW, "Ende des Tages liegt nicht vor jetzt");
        assert!(ts - NOW < 24 * 3600);
        // Deutsche Alias-Form liefert denselben Wert.
        assert_eq!(parse_due_date("heute", NOW), Some(ts));
    }

    #[test]
    fn due_tomorrow_after_today() {
        let today = parse_due_date("today", NOW).unwrap();
        let tomorrow = parse_due_date("tomorrow", NOW).unwrap();
        assert_eq!(tomorrow - today, 24 * 3600);
        assert_eq!(parse_due_date("morgen", NOW), Some(tomorrow));
    }

    #[test]
    fn due_relative_days_weeks() {
        let today = parse_due_date("today", NOW).unwrap();
        assert_eq!(parse_due_date("+3d", NOW).unwrap() - today, 3 * 24 * 3600);
        assert_eq!(parse_due_date("+2w", NOW).unwrap() - today, 14 * 24 * 3600);
    }

    #[test]
    fn due_iso_date() {
        let ts = parse_due_date("2027-01-15", NOW).unwrap();
        let date = Local.timestamp_opt(ts, 0).single().unwrap().date_naive();
        assert_eq!(date, NaiveDate::from_ymd_opt(2027, 1, 15).unwrap());
    }

    #[test]
    fn due_invalid_is_none() {
        assert_eq!(parse_due_date("irgendwann", NOW), None);
        assert_eq!(parse_due_date("+3x", NOW), None);
        assert_eq!(parse_due_date("", NOW), None);
    }

    // ── RecurParser ─────────────────────────────────────────────────────────

    #[test]
    fn recur_standard_words() {
        let due = NOW;
        assert_eq!(next_due_after("daily", due).unwrap() - due, 24 * 3600);
        assert_eq!(next_due_after("weekly", due).unwrap() - due, 7 * 24 * 3600);
        assert!(next_due_after("monthly", due).is_some());
        assert!(next_due_after("yearly", due).is_some());
    }

    #[test]
    fn recur_n_units() {
        let due = NOW;
        assert_eq!(next_due_after("3d", due).unwrap() - due, 3 * 24 * 3600);
        assert_eq!(next_due_after("2w", due).unwrap() - due, 14 * 24 * 3600);
        assert!(next_due_after("6m", due).is_some());
        assert!(next_due_after("1y", due).is_some());
    }

    #[test]
    fn recur_invalid_is_none() {
        assert_eq!(next_due_after("fortnightly", NOW), None);
        assert_eq!(next_due_after("0d", NOW), None);
        assert_eq!(next_due_after("", NOW), None);
        assert!(!is_valid_recur("quatsch"));
        assert!(is_valid_recur("weekly"));
    }

    #[test]
    fn recur_taskwarrior_synonyms() {
        let due = NOW;
        assert_eq!(next_due_after("biweekly", due), next_due_after("2w", due));
        assert_eq!(next_due_after("fortnight", due), next_due_after("2w", due));
        assert_eq!(next_due_after("quarterly", due), next_due_after("3m", due));
        assert_eq!(next_due_after("semiannual", due), next_due_after("6m", due));
        assert_eq!(next_due_after("annual", due), next_due_after("yearly", due));
        assert_eq!(next_due_after("biannual", due), next_due_after("2y", due));
        assert_eq!(next_due_after("3wks", due), next_due_after("3w", due));
        assert_eq!(next_due_after("2mo", due), next_due_after("2m", due));
        assert_eq!(next_due_after("1qtr", due), next_due_after("3m", due));
        assert_eq!(next_due_after("2yrs", due), next_due_after("2y", due));
        assert!(is_valid_recur("weekdays"));
        assert!(!is_valid_recur("9hrs")); // sub-täglich bewusst nicht unterstützt
    }

    #[test]
    fn recur_weekdays_skips_weekend() {
        // 2026-07-17 ist ein Freitag → nächster Werktag ist Montag, 2026-07-20.
        let friday = Local.with_ymd_and_hms(2026, 7, 17, 12, 0, 0).unwrap().timestamp();
        let next = next_due_after("weekdays", friday).unwrap();
        let next_local = Local.timestamp_opt(next, 0).unwrap();
        assert_eq!(next_local.weekday(), Weekday::Mon);
        assert_eq!(next - friday, 3 * 24 * 3600);
        // Mitten in der Woche: einfach +1 Tag.
        let tuesday = Local.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap().timestamp();
        assert_eq!(next_due_after("weekdays", tuesday).unwrap() - tuesday, 24 * 3600);
    }
}
