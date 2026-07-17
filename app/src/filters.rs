//! Sidebar-Filter, Sortierung und Suche — Port des `TaskListViewModel` der
//! macOS-Version. Zentrale Filter-Logik: Sidebar-Counts UND sichtbare Liste
//! nutzen dieselbe `matches`-Funktion, damit nichts driften kann.

use vergissmeinnicht_core::{TaskInfo, TaskStatus};

/// Filter-Modi der Sidebar (Pendant zu `SidebarFilter` in Swift).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarFilter {
    All,
    Today,
    Todo,
    Inbox,
    Overdue,
    DueSoon,
    Upcoming,
    Waiting,
    /// Native Taskwarrior-Abhängigkeits-Reports (`+BLOCKED`/`+BLOCKING`/`+UNBLOCKED`).
    Blocked,
    Blocking,
    Unblocked,
    Project(String),
    Tag(String),
    /// Gespeicherte Suche: Inhalt liefert die parallel gesetzte Suchanfrage;
    /// `matches` liefert `true`, damit die Sidebar-Selektion eindeutig ist.
    SavedSearch(String),
}

impl SidebarFilter {
    /// Serialisierung für die QML-Grenze: `"inbox"`, `"project:Arbeit"`, `"tag:x"`,
    /// `"saved:<id>"`.
    pub fn to_key(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Today => "today".into(),
            Self::Todo => "todo".into(),
            Self::Inbox => "inbox".into(),
            Self::Overdue => "overdue".into(),
            Self::DueSoon => "duesoon".into(),
            Self::Upcoming => "upcoming".into(),
            Self::Waiting => "waiting".into(),
            Self::Blocked => "blocked".into(),
            Self::Blocking => "blocking".into(),
            Self::Unblocked => "unblocked".into(),
            Self::Project(name) => format!("project:{name}"),
            Self::Tag(name) => format!("tag:{name}"),
            Self::SavedSearch(id) => format!("saved:{id}"),
        }
    }

    pub fn from_key(key: &str) -> Self {
        if let Some(name) = key.strip_prefix("project:") {
            return Self::Project(name.to_string());
        }
        if let Some(name) = key.strip_prefix("tag:") {
            return Self::Tag(name.to_string());
        }
        if let Some(id) = key.strip_prefix("saved:") {
            return Self::SavedSearch(id.to_string());
        }
        match key {
            "all" => Self::All,
            "today" => Self::Today,
            "todo" => Self::Todo,
            "overdue" => Self::Overdue,
            "duesoon" => Self::DueSoon,
            "upcoming" => Self::Upcoming,
            "waiting" => Self::Waiting,
            "blocked" => Self::Blocked,
            "blocking" => Self::Blocking,
            "unblocked" => Self::Unblocked,
            _ => Self::Inbox,
        }
    }

    /// Zentrale Filter-Logik (1:1-Port der Swift-Semantik).
    ///
    /// `.Recurring`-Tasks (Master-Vorlagen) brauchen keine explizite Behandlung:
    /// alle actionable Filter gatten bereits auf `status == Pending`, das Recurring
    /// ausschließt. `All`, `Project` und `Tag` zeigen Recurring automatisch.
    pub fn matches(&self, task: &TaskInfo, now: i64, due_soon_days: i64) -> bool {
        match self {
            Self::All => true,
            Self::Today => {
                // "Heute machbar": pending + nicht versteckt + (überfällig ODER fällig
                // heute ODER scheduled heute/vorbei und kein due).
                if task.status != TaskStatus::Pending
                    || is_waiting(task, now)
                    || is_upcoming(task, now)
                {
                    return false;
                }
                if let Some(due) = task.due {
                    // Strikt vor Mitternacht des Folgetags (lokale Zeit): heute fällige
                    // und überfällige zählen; exakt 00:00 morgen gehört zu „morgen".
                    return due < end_of_today_exclusive(now);
                }
                // Kein `due`, aber `scheduled` gesetzt: der `!is_upcoming`-Guard oben
                // hat alles mit `scheduled > now` ausgeschlossen — ein vorhandenes
                // `scheduled` liegt also heute oder in der Vergangenheit.
                task.scheduled.is_some()
            }
            Self::Todo => {
                task.status == TaskStatus::Pending
                    && !is_waiting(task, now)
                    && !is_upcoming(task, now)
            }
            Self::Inbox => {
                task.status == TaskStatus::Pending
                    && task.project.is_none()
                    && task.tags.is_empty()
                    && !is_waiting(task, now)
                    && !is_upcoming(task, now)
            }
            Self::Overdue => {
                task.status == TaskStatus::Pending
                    && !is_upcoming(task, now)
                    && task.due.map(|due| due < now).unwrap_or(false)
            }
            Self::DueSoon => {
                if task.status != TaskStatus::Pending || is_upcoming(task, now) {
                    return false;
                }
                match task.due {
                    Some(due) => due >= now && due <= now + due_soon_days * 24 * 60 * 60,
                    None => false,
                }
            }
            Self::Upcoming => task.status == TaskStatus::Pending && is_upcoming(task, now),
            Self::Waiting => task.status == TaskStatus::Pending && is_waiting(task, now),
            Self::Blocked => task.status == TaskStatus::Pending && task.is_blocked,
            Self::Blocking => task.status == TaskStatus::Pending && task.is_blocking,
            Self::Unblocked => task.status == TaskStatus::Pending && !task.is_blocked,
            Self::Project(name) => project_matches(task.project.as_deref(), name),
            Self::Tag(name) => task.tags.iter().any(|t| t == name),
            Self::SavedSearch(_) => true,
        }
    }
}

/// Taskwarrior-Präfix-Match für Projekte: `project:Work` matcht `Work` UND alle
/// `Work.*`-Subprojekte. Die `.`-Grenze verhindert, dass `Work` auch `Workshop` matcht.
pub fn project_matches(task_project: Option<&str>, selected: &str) -> bool {
    match task_project {
        Some(p) => p == selected || p.starts_with(&format!("{selected}.")),
        None => false,
    }
}

pub fn is_waiting(task: &TaskInfo, now: i64) -> bool {
    task.wait.map(|w| w > now).unwrap_or(false)
}

/// Geplant für die Zukunft — Task hat `scheduled` gesetzt und der Zeitpunkt liegt nach jetzt.
pub fn is_upcoming(task: &TaskInfo, now: i64) -> bool {
    task.scheduled.map(|s| s > now).unwrap_or(false)
}

/// Mitternacht des Folgetags in lokaler Zeit als Unix-Sekunden (exklusive Obergrenze
/// für „heute"). Pendant zu `cal.startOfDay(now) + 24h` in Swift.
pub fn end_of_today_exclusive(now: i64) -> i64 {
    start_of_local_day(now) + 24 * 60 * 60
}

/// Beginn des lokalen Tages (00:00) für einen Unix-Zeitpunkt.
///
/// Bewusst über `chrono::Local` statt Handarithmetik, damit Zeitzonen- und
/// DST-Wechsel korrekt behandelt werden.
pub fn start_of_local_day(now: i64) -> i64 {
    use vergissmeinnicht_core::chrono::{Local, LocalResult, TimeZone};
    let local = match Local.timestamp_opt(now, 0) {
        LocalResult::Single(dt) => dt,
        _ => return now - now.rem_euclid(24 * 60 * 60),
    };
    let date = local.date_naive();
    match date.and_hms_opt(0, 0, 0) {
        Some(naive) => match Local.from_local_datetime(&naive) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.timestamp(),
            LocalResult::None => now - now.rem_euclid(24 * 60 * 60),
        },
        None => now - now.rem_euclid(24 * 60 * 60),
    }
}

// ─── Sortierung ─────────────────────────────────────────────────────────────

/// Sortier-Reihenfolge der Task-Liste (Pendant zu `SortOrder` in Swift).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Id,
    Description,
    Entry,
    Due,
    Project,
}

impl SortOrder {
    pub fn from_key(key: &str) -> Self {
        match key {
            "description" => Self::Description,
            "entry" => Self::Entry,
            "due" => Self::Due,
            "project" => Self::Project,
            _ => Self::Id,
        }
    }

    pub fn to_key(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Description => "description",
            Self::Entry => "entry",
            Self::Due => "due",
            Self::Project => "project",
        }
    }
}

fn ci_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Comparator-Port aus Swift: Options nach hinten, `entry` neueste zuerst,
/// Sekundärschlüssel Description (case-insensitive).
pub fn sort_tasks(tasks: &mut [TaskInfo], order: SortOrder, ascending: bool) {
    use std::cmp::Ordering;
    tasks.sort_by(|lhs, rhs| {
        let ord = match order {
            SortOrder::Id => match (lhs.working_set_id, rhs.working_set_id) {
                (Some(l), Some(r)) => l.cmp(&r),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => ci_cmp(&lhs.description, &rhs.description),
            },
            SortOrder::Description => ci_cmp(&lhs.description, &rhs.description),
            SortOrder::Entry => match (lhs.entry, rhs.entry) {
                // Neueste zuerst.
                (Some(l), Some(r)) => r.cmp(&l),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => ci_cmp(&lhs.description, &rhs.description),
            },
            SortOrder::Due => match (lhs.due, rhs.due) {
                (Some(l), Some(r)) => l.cmp(&r),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => ci_cmp(&lhs.description, &rhs.description),
            },
            SortOrder::Project => match (lhs.project.as_deref(), rhs.project.as_deref()) {
                (Some(l), Some(r)) => {
                    let cmp = ci_cmp(l, r);
                    if cmp != Ordering::Equal {
                        cmp
                    } else {
                        ci_cmp(&lhs.description, &rhs.description)
                    }
                }
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => ci_cmp(&lhs.description, &rhs.description),
            },
        };
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}

// ─── Suche ──────────────────────────────────────────────────────────────────

/// Geparste Suchanfrage. Alle Bedingungen sind AND-verknüpft.
#[derive(Debug, Default, PartialEq)]
pub struct ParsedQuery {
    pub free_terms: Vec<String>,
    pub projects: Vec<String>,
    pub tags: Vec<String>,
    pub statuses: Vec<TaskStatus>,
}

/// Parst die Suchanfrage in ein typisiertes Modell oder gibt `None` zurück, wenn
/// die Eingabe leer ist. Operatoren: `project:`, `tag:`, `status:` jeweils mit
/// deutschen Aliasen (`projekt:`, `status:offen` …). Werte mit Leerzeichen können
/// in doppelten Anführungszeichen stehen.
pub fn parse_search_query(input: &str) -> Option<ParsedQuery> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut result = ParsedQuery::default();
    for token in tokenize_quoted(trimmed) {
        if let Some((key, value)) = split_operator(&token) {
            match key.to_lowercase().as_str() {
                "project" | "projekt" => result.projects.push(value.to_string()),
                "tag" => result.tags.push(value.to_string()),
                "status" => {
                    if let Some(status) = parse_status(value) {
                        result.statuses.push(status);
                    } else {
                        // Unbekannter Status-Wert → als Freitext werten, damit der
                        // User nicht stumm leere Ergebnisse bekommt.
                        result.free_terms.push(token.clone());
                    }
                }
                // Unbekannter Operator-Key → kompletter Token als Freitext.
                _ => result.free_terms.push(token.clone()),
            }
        } else {
            result.free_terms.push(token);
        }
    }
    Some(result)
}

fn tokenize_quoted(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Splittet an erstem `:` und liefert (key, value), sofern beide nicht leer sind.
fn split_operator(token: &str) -> Option<(&str, &str)> {
    let colon = token.find(':')?;
    let (key, rest) = token.split_at(colon);
    let value = &rest[1..];
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn parse_status(value: &str) -> Option<TaskStatus> {
    match value.to_lowercase().as_str() {
        "pending" | "offen" | "open" => Some(TaskStatus::Pending),
        "completed" | "done" | "erledigt" => Some(TaskStatus::Completed),
        "deleted" | "gelöscht" | "geloescht" => Some(TaskStatus::Deleted),
        "recurring" | "wiederkehrend" => Some(TaskStatus::Recurring),
        _ => None,
    }
}

fn ci_contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

pub fn query_matches(task: &TaskInfo, query: &ParsedQuery) -> bool {
    if !query.statuses.is_empty() && !query.statuses.contains(&task.status) {
        return false;
    }
    for project in &query.projects {
        match task.project.as_deref() {
            Some(p) if p.to_lowercase() == project.to_lowercase() => {}
            _ => return false,
        }
    }
    for tag in &query.tags {
        if !task.tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
            return false;
        }
    }
    if query.free_terms.is_empty() {
        return true;
    }
    // Alle Felder, in denen freier Suchtext suchen darf.
    let mut haystacks: Vec<String> = vec![task.description.clone()];
    if let Some(project) = &task.project {
        haystacks.push(project.clone());
    }
    if !task.tags.is_empty() {
        haystacks.push(task.tags.join(" "));
    }
    if !task.annotations.is_empty() {
        haystacks.push(
            task.annotations
                .iter()
                .map(|a| a.description.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    for term in &query.free_terms {
        if !haystacks.iter().any(|h| ci_contains(h, term)) {
            return false;
        }
    }
    true
}

// ─── Projekt-/Tag-Pools ─────────────────────────────────────────────────────

fn is_active(task: &TaskInfo) -> bool {
    task.status == TaskStatus::Pending || task.status == TaskStatus::Recurring
}

/// Projekte aus dem aktiven Task-Pool (Pending + Recurring-Master), alphabetisch.
/// Completed werden ignoriert, damit abgeräumte Projekte nicht ewig in der Sidebar
/// bleiben. Recurring zählt mit, sonst wäre ein Projekt mit ausschließlich
/// Recurring-Master über die Sidebar nicht erreichbar.
pub fn projects_from(tasks: &[TaskInfo]) -> Vec<String> {
    let mut set: Vec<String> = tasks
        .iter()
        .filter(|t| is_active(t))
        .filter_map(|t| t.project.clone())
        .collect();
    set.sort_by(|a, b| ci_cmp(a, b));
    set.dedup();
    set
}

/// Tags aus dem aktiven Task-Pool (Pending + Recurring-Master), alphabetisch.
pub fn tags_from(tasks: &[TaskInfo]) -> Vec<String> {
    let mut set: Vec<String> = tasks
        .iter()
        .filter(|t| is_active(t))
        .flat_map(|t| t.tags.iter().cloned())
        .collect();
    set.sort_by(|a, b| ci_cmp(a, b));
    set.dedup();
    set
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: TaskStatus) -> TaskInfo {
        TaskInfo {
            uuid: "u".into(),
            description: "Test".into(),
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
        }
    }

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn inbox_requires_no_project_no_tags() {
        let mut t = task(TaskStatus::Pending);
        assert!(SidebarFilter::Inbox.matches(&t, NOW, 7));
        t.project = Some("x".into());
        assert!(!SidebarFilter::Inbox.matches(&t, NOW, 7));
        t.project = None;
        t.tags = vec!["a".into()];
        assert!(!SidebarFilter::Inbox.matches(&t, NOW, 7));
    }

    #[test]
    fn waiting_hides_from_todo_and_inbox() {
        let mut t = task(TaskStatus::Pending);
        t.wait = Some(NOW + 3600);
        assert!(SidebarFilter::Waiting.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Todo.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Inbox.matches(&t, NOW, 7));
        // Abgelaufener wait zählt nicht mehr als wartend.
        t.wait = Some(NOW - 3600);
        assert!(!SidebarFilter::Waiting.matches(&t, NOW, 7));
        assert!(SidebarFilter::Todo.matches(&t, NOW, 7));
    }

    #[test]
    fn upcoming_hides_from_actionable_views() {
        let mut t = task(TaskStatus::Pending);
        t.scheduled = Some(NOW + 86_400);
        assert!(SidebarFilter::Upcoming.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Todo.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Today.matches(&t, NOW, 7));
        t.due = Some(NOW - 100);
        assert!(!SidebarFilter::Overdue.matches(&t, NOW, 7));
    }

    #[test]
    fn today_includes_overdue_and_scheduled_today() {
        let mut t = task(TaskStatus::Pending);
        t.due = Some(NOW - 100);
        assert!(SidebarFilter::Today.matches(&t, NOW, 7));
        // due morgen (nach Mitternacht des Folgetags) zählt nicht.
        t.due = Some(end_of_today_exclusive(NOW) + 10);
        assert!(!SidebarFilter::Today.matches(&t, NOW, 7));
        // Kein due, aber scheduled in der Vergangenheit → heute machbar.
        t.due = None;
        t.scheduled = Some(NOW - 100);
        assert!(SidebarFilter::Today.matches(&t, NOW, 7));
    }

    #[test]
    fn due_soon_window() {
        let mut t = task(TaskStatus::Pending);
        t.due = Some(NOW + 3 * 86_400);
        assert!(SidebarFilter::DueSoon.matches(&t, NOW, 7));
        assert!(!SidebarFilter::DueSoon.matches(&t, NOW, 2));
        // Überfällig ist nicht "bald fällig".
        t.due = Some(NOW - 1);
        assert!(!SidebarFilter::DueSoon.matches(&t, NOW, 7));
    }

    #[test]
    fn project_prefix_semantics() {
        assert!(project_matches(Some("Work"), "Work"));
        assert!(project_matches(Some("Work.Sub"), "Work"));
        assert!(!project_matches(Some("Workshop"), "Work"));
        assert!(!project_matches(None, "Work"));
    }

    #[test]
    fn blocked_blocking_unblocked() {
        let mut t = task(TaskStatus::Pending);
        t.is_blocked = true;
        assert!(SidebarFilter::Blocked.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Unblocked.matches(&t, NOW, 7));
        t.is_blocked = false;
        t.is_blocking = true;
        assert!(SidebarFilter::Blocking.matches(&t, NOW, 7));
        assert!(SidebarFilter::Unblocked.matches(&t, NOW, 7));
    }

    #[test]
    fn recurring_visible_only_in_unfiltered_views() {
        let mut t = task(TaskStatus::Recurring);
        t.project = Some("routine".into());
        assert!(SidebarFilter::All.matches(&t, NOW, 7));
        assert!(SidebarFilter::Project("routine".into()).matches(&t, NOW, 7));
        assert!(!SidebarFilter::Todo.matches(&t, NOW, 7));
        assert!(!SidebarFilter::Today.matches(&t, NOW, 7));
    }

    #[test]
    fn filter_key_roundtrip() {
        for f in [
            SidebarFilter::All,
            SidebarFilter::Today,
            SidebarFilter::Inbox,
            SidebarFilter::Project("Mit Leerzeichen".into()),
            SidebarFilter::Tag("dringend".into()),
            SidebarFilter::SavedSearch("abc-123".into()),
        ] {
            assert_eq!(SidebarFilter::from_key(&f.to_key()), f);
        }
    }

    #[test]
    fn sort_by_id_none_last() {
        let mut a = task(TaskStatus::Pending);
        a.working_set_id = Some(2);
        a.description = "B".into();
        let mut b = task(TaskStatus::Completed);
        b.working_set_id = None;
        b.description = "A".into();
        let mut c = task(TaskStatus::Pending);
        c.working_set_id = Some(1);
        c.description = "C".into();
        let mut v = vec![a, b, c];
        sort_tasks(&mut v, SortOrder::Id, true);
        assert_eq!(
            v.iter().map(|t| t.working_set_id).collect::<Vec<_>>(),
            vec![Some(1), Some(2), None]
        );
    }

    #[test]
    fn sort_entry_newest_first() {
        let mut a = task(TaskStatus::Pending);
        a.entry = Some(100);
        let mut b = task(TaskStatus::Pending);
        b.entry = Some(200);
        let mut v = vec![a, b];
        sort_tasks(&mut v, SortOrder::Entry, true);
        assert_eq!(v[0].entry, Some(200));
    }

    #[test]
    fn search_operators_and_quotes() {
        let q = parse_search_query("projekt:Arbeit tag:x status:erledigt \"zwei worte\" rest").unwrap();
        assert_eq!(q.projects, vec!["Arbeit"]);
        assert_eq!(q.tags, vec!["x"]);
        assert_eq!(q.statuses, vec![TaskStatus::Completed]);
        assert_eq!(q.free_terms, vec!["zwei worte", "rest"]);
    }

    #[test]
    fn search_unknown_operator_is_free_text() {
        let q = parse_search_query("foo:bar status:quatsch").unwrap();
        assert_eq!(q.free_terms, vec!["foo:bar", "status:quatsch"]);
    }

    #[test]
    fn search_empty_is_none() {
        assert!(parse_search_query("   ").is_none());
    }

    #[test]
    fn query_matches_all_fields() {
        let mut t = task(TaskStatus::Pending);
        t.description = "Rechnung schreiben".into();
        t.project = Some("Büro".into());
        t.tags = vec!["finanzen".into()];
        t.annotations = vec![vergissmeinnicht_core::AnnotationInfo {
            entry: 1,
            description: "Vorlage im Ordner".into(),
        }];

        let q = parse_search_query("rechnung").unwrap();
        assert!(query_matches(&t, &q));
        let q = parse_search_query("projekt:büro").unwrap();
        assert!(query_matches(&t, &q));
        let q = parse_search_query("tag:finanzen vorlage").unwrap();
        assert!(query_matches(&t, &q));
        let q = parse_search_query("tag:anderes").unwrap();
        assert!(!query_matches(&t, &q));
        let q = parse_search_query("status:erledigt").unwrap();
        assert!(!query_matches(&t, &q));
    }

    #[test]
    fn projects_and_tags_pools_ignore_completed() {
        let mut a = task(TaskStatus::Pending);
        a.project = Some("Aktiv".into());
        a.tags = vec!["t1".into()];
        let mut b = task(TaskStatus::Completed);
        b.project = Some("Fertig".into());
        b.tags = vec!["t2".into()];
        let mut c = task(TaskStatus::Recurring);
        c.project = Some("Routine".into());
        let v = vec![a, b, c];
        assert_eq!(projects_from(&v), vec!["Aktiv", "Routine"]);
        assert_eq!(tags_from(&v), vec!["t1"]);
    }
}
