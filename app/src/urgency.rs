//! Taskwarrior-Urgency — Nachbau der CLI-Formel mit den Default-Koeffizienten
//! von Taskwarrior 3.4.2 (`task show urgency`, `src/Task.cpp::urgency_c`).
//! Bewusst ohne Konfigurierbarkeit: die Defaults sind der De-facto-Standard,
//! und die Zahl muss mit der CLI-Anzeige übereinstimmen.

use vergissmeinnicht_core::{TaskInfo, TaskStatus};

const COEF_ACTIVE: f64 = 4.0;
const COEF_AGE: f64 = 2.0;
const AGE_MAX_DAYS: f64 = 365.0;
const COEF_ANNOTATIONS: f64 = 1.0;
const COEF_BLOCKED: f64 = -5.0;
const COEF_BLOCKING: f64 = 8.0;
const COEF_DUE: f64 = 12.0;
const COEF_PROJECT: f64 = 1.0;
const COEF_SCHEDULED: f64 = 5.0;
const COEF_TAGS: f64 = 1.0;
const COEF_WAITING: f64 = -3.0;
const COEF_PRIO_H: f64 = 6.0;
const COEF_PRIO_M: f64 = 3.9;
const COEF_PRIO_L: f64 = 1.8;
const COEF_TAG_NEXT: f64 = 15.0;

/// Urgency eines Tasks zum Zeitpunkt `now` (Unix-Sekunden). Nicht-Pending-Tasks
/// haben Urgency 0 (wie in CLI-Reports, die nur Pending listen).
pub fn urgency(t: &TaskInfo, now: i64) -> f64 {
    if t.status != TaskStatus::Pending {
        return 0.0;
    }
    let mut u = 0.0;
    if t.project.is_some() {
        u += COEF_PROJECT;
    }
    if t.start.is_some() {
        u += COEF_ACTIVE;
    }
    // Scheduled zählt nur, wenn der Task tatsächlich „ready" ist (Datum vorbei).
    if t.scheduled.is_some_and(|s| s <= now) {
        u += COEF_SCHEDULED;
    }
    if t.wait.is_some_and(|w| w > now) {
        u += COEF_WAITING;
    }
    if t.is_blocked {
        u += COEF_BLOCKED;
    }
    if t.is_blocking {
        u += COEF_BLOCKING;
    }
    u += count_factor(t.annotations.len()) * COEF_ANNOTATIONS;
    u += count_factor(t.tags.len()) * COEF_TAGS;
    if let Some(due) = t.due {
        u += due_factor(due, now) * COEF_DUE;
    }
    match t.entry {
        Some(entry) => {
            let age_days = ((now - entry) as f64 / 86400.0).max(0.0);
            let factor = if age_days > AGE_MAX_DAYS { 1.0 } else { age_days / AGE_MAX_DAYS };
            u += factor * COEF_AGE;
        }
        None => u += COEF_AGE,
    }
    match t.priority.as_deref() {
        Some("H") => u += COEF_PRIO_H,
        Some("M") => u += COEF_PRIO_M,
        Some("L") => u += COEF_PRIO_L,
        _ => {}
    }
    if t.tags.iter().any(|tag| tag == "next") {
        u += COEF_TAG_NEXT;
    }
    u
}

/// Stufenfaktor für Tag-/Annotation-Anzahl (CLI: 0.8 / 0.9 / 1.0).
fn count_factor(n: usize) -> f64 {
    match n {
        0 => 0.0,
        1 => 0.8,
        2 => 0.9,
        _ => 1.0,
    }
}

/// Fälligkeits-Rampe: 0.2 (weit weg) bis 1.0 (≥ 7 Tage überfällig).
fn due_factor(due: i64, now: i64) -> f64 {
    let days_overdue = (now - due) as f64 / 86400.0;
    if days_overdue >= 7.0 {
        1.0
    } else if days_overdue >= -14.0 {
        ((days_overdue + 14.0) * 0.8 / 21.0) + 0.2
    } else {
        0.2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vergissmeinnicht_core::TaskInfo;

    fn task() -> TaskInfo {
        TaskInfo {
            uuid: "u".into(),
            description: "t".into(),
            project: None,
            tags: vec![],
            due: None,
            status: TaskStatus::Pending,
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

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn bare_task_scores_age_coefficient_only() {
        // Ohne entry zählt der volle Age-Term (CLI-Verhalten).
        assert!((urgency(&task(), NOW) - COEF_AGE).abs() < 1e-9);
    }

    #[test]
    fn priority_and_project_and_next_tag() {
        let mut t = task();
        t.entry = Some(NOW); // Age-Faktor 0
        t.priority = Some("H".into());
        t.project = Some("p".into());
        t.tags = vec!["next".into()];
        // H (6.0) + Projekt (1.0) + 1 Tag (0.8*1.0) + next (15.0)
        assert!((urgency(&t, NOW) - (6.0 + 1.0 + 0.8 + 15.0)).abs() < 1e-9);
    }

    #[test]
    fn due_tomorrow_matches_cli_factor() {
        let mut t = task();
        t.entry = Some(NOW);
        t.due = Some(NOW + 86400); // morgen → days_overdue = -1
        let expected = ((-1.0 + 14.0) * 0.8 / 21.0) + 0.2; // ≈ 0.695
        assert!((urgency(&t, NOW) - expected * COEF_DUE).abs() < 1e-9);
    }

    #[test]
    fn tag_count_steps() {
        let mut t = task();
        t.entry = Some(NOW);
        t.tags = vec!["a".into()];
        assert!((urgency(&t, NOW) - 0.8).abs() < 1e-9);
        t.tags.push("b".into());
        assert!((urgency(&t, NOW) - 0.9).abs() < 1e-9);
        t.tags.push("c".into());
        assert!((urgency(&t, NOW) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn blocked_and_waiting_are_negative() {
        let mut t = task();
        t.entry = Some(NOW);
        t.is_blocked = true;
        t.wait = Some(NOW + 86400);
        assert!((urgency(&t, NOW) - (COEF_BLOCKED + COEF_WAITING)).abs() < 1e-9);
    }

    #[test]
    fn scheduled_counts_only_when_ready() {
        let mut t = task();
        t.entry = Some(NOW);
        t.scheduled = Some(NOW + 86400); // Zukunft → kein Term
        assert!((urgency(&t, NOW)).abs() < 1e-9);
        t.scheduled = Some(NOW - 86400); // vorbei → +5.0
        assert!((urgency(&t, NOW) - COEF_SCHEDULED).abs() < 1e-9);
    }

    #[test]
    fn non_pending_is_zero() {
        let mut t = task();
        t.priority = Some("H".into());
        t.status = TaskStatus::Completed;
        assert_eq!(urgency(&t, NOW), 0.0);
    }
}
