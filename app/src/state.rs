//! Zentraler App-Zustand — Pendant zum macOS-`AppContainer` (Single Source of
//! Truth) plus dem abgeleiteten UI-State des `TaskListViewModel`. Qt-frei und
//! damit direkt testbar; die cxx-qt-Bridge (`bridge.rs`) ist nur Übersetzung.

use std::sync::Arc;

use serde_json::json;
use vergissmeinnicht_core::{TaskInfo, TaskStatus, TaskStore};

use crate::config::{SavedSearch, Settings};
use crate::filters::{
    parse_search_query, projects_from, query_matches, sort_tasks, tags_from, SidebarFilter,
    SortOrder,
};
use crate::{backup, parsers};

pub struct AppState {
    pub store: Option<Arc<TaskStore>>,
    pub init_error: Option<String>,
    /// Alle sichtbaren Tasks (Pending + Completed + Recurring), Quelle für alles.
    pub tasks: Vec<TaskInfo>,
    /// Gefilterte + sortierte Sicht für das Listenmodell.
    pub visible: Vec<TaskInfo>,
    pub filter: SidebarFilter,
    pub search_query: String,
    pub sort: SortOrder,
    pub sort_ascending: bool,
    pub settings: Settings,
}

fn now_secs() -> i64 {
    vergissmeinnicht_core::chrono::Utc::now().timestamp()
}

impl AppState {
    pub fn init() -> Self {
        let settings = Settings::load();
        let replica_dir = crate::config::replica_dir();
        let (store, init_error) = match std::fs::create_dir_all(&replica_dir) {
            Err(e) => (None, Some(format!("Replica-Verzeichnis: {e}"))),
            Ok(()) => match TaskStore::new(replica_dir.to_string_lossy().into_owned()) {
                Ok(s) => (Some(Arc::new(s)), None),
                Err(e) => (None, Some(e.to_string())),
            },
        };
        let mut state = Self {
            store,
            init_error,
            tasks: Vec::new(),
            visible: Vec::new(),
            filter: SidebarFilter::from_key(&settings.default_filter),
            search_query: String::new(),
            sort: SortOrder::from_key(&settings.sort_key),
            sort_ascending: settings.sort_ascending,
            settings,
        };
        let _ = state.refresh();
        state
    }

    /// Lädt alle Tasks neu aus dem Store und baut die sichtbare Liste.
    pub fn refresh(&mut self) -> Result<(), String> {
        let Some(store) = &self.store else {
            return Err(self
                .init_error
                .clone()
                .unwrap_or_else(|| "Store nicht initialisiert".into()));
        };
        match store.list_tasks(true) {
            Ok(tasks) => {
                self.tasks = tasks;
                self.rebuild_visible();
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Filter + Suche + hideCompleted + Sortierung — Port von `visibleTasks`.
    /// Mit aktiver Suche wechselt der Scope: Sidebar-Filter und hideCompleted
    /// werden ignoriert, damit die Suche bestandsweit arbeitet.
    pub fn rebuild_visible(&mut self) {
        let now = now_secs();
        let mut filtered: Vec<TaskInfo> = if let Some(query) = parse_search_query(&self.search_query)
        {
            self.tasks
                .iter()
                .filter(|t| query_matches(t, &query))
                .cloned()
                .collect()
        } else {
            self.tasks
                .iter()
                .filter(|t| self.filter.matches(t, now, self.settings.due_soon_days))
                .filter(|t| !self.settings.hide_completed || t.status != TaskStatus::Completed)
                .cloned()
                .collect()
        };
        sort_tasks(&mut filtered, self.sort, self.sort_ascending);
        self.visible = filtered;
    }

    pub fn task_by_uuid(&self, uuid: &str) -> Option<&TaskInfo> {
        self.tasks.iter().find(|t| t.uuid == uuid)
    }

    // ─── Sidebar-Daten (JSON für QML) ────────────────────────────────────────

    pub fn counts_json(&self) -> String {
        let now = now_secs();
        let days = self.settings.due_soon_days;
        let count = |f: &SidebarFilter| {
            self.tasks
                .iter()
                .filter(|t| f.matches(t, now, days))
                .count()
        };
        // "Zu erledigen"-Zähler bewusst ohne Wartende (macOS todoCount-Bugfix).
        json!({
            "inbox": count(&SidebarFilter::Inbox),
            "today": count(&SidebarFilter::Today),
            "todo": count(&SidebarFilter::Todo),
            "overdue": count(&SidebarFilter::Overdue),
            "duesoon": count(&SidebarFilter::DueSoon),
            "upcoming": count(&SidebarFilter::Upcoming),
            "waiting": count(&SidebarFilter::Waiting),
            "all": self.tasks.len(),
            "blocked": count(&SidebarFilter::Blocked),
            "blocking": count(&SidebarFilter::Blocking),
        })
        .to_string()
    }

    /// Projekte als flache, hierarchisch sortierte Liste mit Tiefe (gepunktete
    /// Taskwarrior-Hierarchie, macOS-#10-Pendant). Implizite Eltern (`Work` bei
    /// `Work.Sub` ohne eigene Aufgaben) werden ergänzt; der Count nutzt die
    /// Präfix-Semantik von `SidebarFilter::Project` und zählt Subprojekte mit.
    pub fn projects_json(&self) -> String {
        let now = now_secs();
        let days = self.settings.due_soon_days;

        // Explizite Projekte + implizite Eltern sammeln.
        let mut names: Vec<String> = projects_from(&self.tasks);
        let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for name in names.drain(..) {
            let parts: Vec<&str> = name.split('.').collect();
            for i in 1..=parts.len() {
                all.insert(parts[..i].join("."));
            }
        }
        // Hierarchische Ordnung: case-insensitiv, Kinder direkt unter Eltern.
        let mut sorted: Vec<String> = all.into_iter().collect();
        sorted.sort_by_key(|n| n.to_lowercase());

        let items: Vec<serde_json::Value> = sorted
            .iter()
            .map(|name| {
                let f = SidebarFilter::Project(name.clone());
                let count = self.tasks.iter().filter(|t| f.matches(t, now, days)).count();
                let depth = name.matches('.').count();
                let label = name.rsplit('.').next().unwrap_or(name).to_string();
                let prefix = format!("{name}.");
                let has_children = sorted.iter().any(|n| n.starts_with(&prefix));
                json!({
                    "name": name,
                    "label": label,
                    "depth": depth,
                    "hasChildren": has_children,
                    "count": count,
                })
            })
            .collect();
        serde_json::Value::Array(items).to_string()
    }

    pub fn tags_json(&self) -> String {
        let now = now_secs();
        let days = self.settings.due_soon_days;
        let items: Vec<serde_json::Value> = tags_from(&self.tasks)
            .into_iter()
            .map(|name| {
                let f = SidebarFilter::Tag(name.clone());
                let count = self.tasks.iter().filter(|t| f.matches(t, now, days)).count();
                json!({"name": name, "count": count})
            })
            .collect();
        serde_json::Value::Array(items).to_string()
    }

    pub fn saved_searches_json(&self) -> String {
        let mut sorted = self.settings.saved_searches.clone();
        sorted.sort_by_key(|a| a.name.to_lowercase());
        serde_json::to_string(&sorted).unwrap_or_else(|_| "[]".into())
    }

    /// Offene Aufgaben (Pending) als kompakte JSON-Liste — Auswahlquelle für
    /// den Abhängigkeits-Editor. Sortiert nach Working-Set-ID.
    pub fn pending_tasks_json(&self) -> String {
        let mut pending: Vec<&TaskInfo> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect();
        pending.sort_by_key(|t| t.working_set_id.unwrap_or(u32::MAX));
        let items: Vec<serde_json::Value> = pending
            .iter()
            .map(|t| {
                json!({
                    "uuid": t.uuid,
                    "title": t.description,
                    "wsId": t.working_set_id,
                })
            })
            .collect();
        serde_json::Value::Array(items).to_string()
    }

    /// Vollständige Task-Details als JSON für den Detail-Editor.
    pub fn task_json(&self, uuid: &str) -> String {
        let Some(t) = self.task_by_uuid(uuid) else {
            return "null".into();
        };
        task_to_json(t).to_string()
    }

    // ─── Mutationen ─────────────────────────────────────────────────────────

    /// Führt eine Store-Mutation aus und lädt danach neu. Fehlertext für die UI.
    pub fn mutate<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&TaskStore) -> Result<(), vergissmeinnicht_core::VmError>,
    {
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        f(store).map_err(|e| e.to_string())?;
        self.refresh()
    }

    /// Erledigt einen Task; bei gesetztem `recur` + `due` wird atomar die
    /// Folge-Instanz erzeugt (Generator-Light, Port von `markDoneWithRecurrence`).
    pub fn mark_done_smart(&mut self, uuid: &str) -> Result<(), String> {
        let Some(task) = self.task_by_uuid(uuid).cloned() else {
            return Err(format!("Task nicht gefunden: {uuid}"));
        };
        let followup_due = match (&task.recur, task.due) {
            (Some(recur), Some(due)) => parsers::next_due_after(recur, due),
            _ => None,
        };
        self.mutate(|store| {
            if let Some(new_due) = followup_due {
                store
                    .mark_done_with_followup(
                        uuid.to_string(),
                        Some(new_due),
                        task.recur.clone(),
                        task.priority.clone(),
                        task.project.clone(),
                        task.tags.clone(),
                        task.description.clone(),
                    )
                    .map(|_| ())
            } else {
                store.mark_done(uuid.to_string())
            }
        })
    }

    /// QuickCapture-Commit: Token-Eingabe parsen und Task anlegen.
    pub fn quick_capture(&mut self, input: &str) -> Result<(), String> {
        let preview = parsers::parse_quick_capture(input);
        if preview.description.trim().is_empty() {
            return Err("Beschreibung darf nicht leer sein".into());
        }
        let due = preview
            .due
            .as_deref()
            .and_then(|d| parsers::parse_due_date(d, now_secs()));
        let priority = preview.priority.clone();
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        let uuid = store
            .add_task_full(
                preview.description.clone(),
                preview.project.clone(),
                preview.tags.clone(),
                due,
            )
            .map_err(|e| e.to_string())?;
        if let Some(p) = priority.filter(|p| !p.is_empty()) {
            store
                .set_priority(uuid, Some(p))
                .map_err(|e| e.to_string())?;
        }
        self.refresh()
    }

    /// Projekt umbenennen über alle betroffenen aktiven Tasks. Teilfehler werden
    /// gesammelt gemeldet. Reihenfolge pro Task: erst neues Projekt setzen (Port
    /// der macOS-Härtung: kein Datenverlust, wenn das Setzen fehlschlägt).
    pub fn rename_project(&mut self, old: &str, new: &str) -> Result<(), String> {
        let affected: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Deleted)
            .filter(|t| t.project.as_deref() == Some(old))
            .map(|t| t.uuid.clone())
            .collect();
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        let mut errors = 0;
        for uuid in affected {
            if store
                .set_project(uuid, Some(new.to_string()))
                .is_err()
            {
                errors += 1;
            }
        }
        let result = self.refresh();
        if errors > 0 {
            return Err(format!("{errors} Aufgabe(n) konnten nicht umbenannt werden"));
        }
        result
    }

    pub fn clear_project(&mut self, name: &str) -> Result<(), String> {
        let affected: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.project.as_deref() == Some(name))
            .map(|t| t.uuid.clone())
            .collect();
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        let mut errors = 0;
        for uuid in affected {
            if store.set_project(uuid, None).is_err() {
                errors += 1;
            }
        }
        let result = self.refresh();
        if errors > 0 {
            return Err(format!("{errors} Aufgabe(n) konnten nicht geändert werden"));
        }
        result
    }

    /// Tag umbenennen: erst neuen Tag setzen, nur bei Erfolg alten entfernen
    /// (Port der macOS-Härtung gegen Tag-Verlust).
    pub fn rename_tag(&mut self, old: &str, new: &str) -> Result<(), String> {
        let affected: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.tags.iter().any(|tag| tag == old))
            .map(|t| t.uuid.clone())
            .collect();
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        let mut errors = 0;
        for uuid in affected {
            match store.add_tag(uuid.clone(), new.to_string()) {
                Ok(()) => {
                    if store.remove_tag(uuid, old.to_string()).is_err() {
                        errors += 1;
                    }
                }
                Err(_) => errors += 1,
            }
        }
        let result = self.refresh();
        if errors > 0 {
            return Err(format!("{errors} Aufgabe(n) konnten nicht umbenannt werden"));
        }
        result
    }

    pub fn clear_tag(&mut self, name: &str) -> Result<(), String> {
        let affected: Vec<String> = self
            .tasks
            .iter()
            .filter(|t| t.tags.iter().any(|tag| tag == name))
            .map(|t| t.uuid.clone())
            .collect();
        let Some(store) = &self.store else {
            return Err("Store nicht initialisiert".into());
        };
        let mut errors = 0;
        for uuid in affected {
            if store.remove_tag(uuid, name.to_string()).is_err() {
                errors += 1;
            }
        }
        let result = self.refresh();
        if errors > 0 {
            return Err(format!("{errors} Aufgabe(n) konnten nicht geändert werden"));
        }
        result
    }

    /// Legacy-Reparatur (Port des macOS-Repair-Laufs): Pending-Aufgaben, deren
    /// Description noch Token-Syntax enthält (`+tag`, `project:`, `due:`,
    /// `priority:` als Text), werden in saubere Properties überführt.
    /// Bestehende Properties gewinnen; Token-Werte füllen nur Lücken.
    /// Gibt die Anzahl reparierter Aufgaben zurück.
    pub fn repair_legacy_tasks(&mut self) -> Result<usize, String> {
        let Some(store) = self.store.clone() else {
            return Err("Store nicht initialisiert".into());
        };
        let now = now_secs();
        let mut repaired = 0;
        let mut errors = 0;
        for task in self.tasks.clone() {
            if task.status != TaskStatus::Pending {
                continue;
            }
            let preview = parsers::parse_quick_capture(&task.description);
            let has_meta = !preview.tags.is_empty()
                || preview.project.is_some()
                || preview.due.is_some()
                || preview.priority.is_some();
            if !has_meta || preview.description.trim().is_empty() {
                continue;
            }
            let project = task.project.clone().or(preview.project);
            let mut tags = task.tags.clone();
            for t in preview.tags {
                if !tags.contains(&t) {
                    tags.push(t);
                }
            }
            let due = task
                .due
                .or_else(|| preview.due.as_deref().and_then(|d| parsers::parse_due_date(d, now)));
            if store
                .update_task_metadata(task.uuid.clone(), preview.description, project, tags, due)
                .is_err()
            {
                errors += 1;
                continue;
            }
            if task.priority.is_none() {
                if let Some(p) = preview.priority {
                    let _ = store.set_priority(task.uuid.clone(), Some(p));
                }
            }
            repaired += 1;
        }
        self.refresh()?;
        if errors > 0 {
            return Err(format!("{errors} Aufgabe(n) konnten nicht repariert werden"));
        }
        Ok(repaired)
    }

    // ─── Saved Searches ─────────────────────────────────────────────────────

    pub fn save_search(&mut self, name: &str, query: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || query.trim().is_empty() {
            return Err("Name und Suchanfrage dürfen nicht leer sein".into());
        }
        if self
            .settings
            .saved_searches
            .iter()
            .any(|s| s.name.to_lowercase() == name.to_lowercase())
        {
            return Err(format!("Suche „{name}“ existiert bereits"));
        }
        let id = uuid_v4_string();
        self.settings.saved_searches.push(SavedSearch {
            id: id.clone(),
            name: name.to_string(),
            query: query.trim().to_string(),
        });
        self.settings.save().map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn rename_saved_search(&mut self, id: &str, new_name: &str) -> Result<(), String> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err("Name darf nicht leer sein".into());
        }
        match self.settings.saved_searches.iter_mut().find(|s| s.id == id) {
            Some(s) => s.name = new_name.to_string(),
            None => return Err("Gespeicherte Suche nicht gefunden".into()),
        }
        self.settings.save().map_err(|e| e.to_string())
    }

    pub fn delete_saved_search(&mut self, id: &str) -> Result<(), String> {
        self.settings.saved_searches.retain(|s| s.id != id);
        self.settings.save().map_err(|e| e.to_string())
    }

    pub fn saved_search_query(&self, id: &str) -> Option<String> {
        self.settings
            .saved_searches
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.query.clone())
    }

    // ─── Sync-Hilfen ────────────────────────────────────────────────────────

    /// Auto-Backup vor Sync (best effort — Fehler blockieren den Sync nicht,
    /// werden aber gemeldet).
    pub fn backup_before_sync() -> Option<String> {
        backup::create_backup(
            &crate::config::replica_dir(),
            &crate::config::backup_dir(),
            "auto",
        )
        .err()
    }

    pub fn overdue_count(&self) -> usize {
        let now = now_secs();
        self.tasks
            .iter()
            .filter(|t| SidebarFilter::Overdue.matches(t, now, self.settings.due_soon_days))
            .count()
    }
}

/// Task als JSON-Objekt (für Detail-Editor und Modell-Zusatzfelder).
pub fn task_to_json(t: &TaskInfo) -> serde_json::Value {
    json!({
        "uuid": t.uuid,
        "description": t.description,
        "project": t.project,
        "tags": t.tags,
        "due": t.due,
        "status": status_key(t.status),
        "entry": t.entry,
        "workingSetId": t.working_set_id,
        "priority": t.priority,
        "annotations": t.annotations.iter().map(|a| json!({
            "entry": a.entry,
            "description": a.description,
        })).collect::<Vec<_>>(),
        "wait": t.wait,
        "recur": t.recur,
        "scheduled": t.scheduled,
        "depends": t.depends,
        "isBlocked": t.is_blocked,
        "isBlocking": t.is_blocking,
    })
}

pub fn status_key(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Completed => "completed",
        TaskStatus::Deleted => "deleted",
        TaskStatus::Recurring => "recurring",
    }
}

/// UUID v4 ohne zusätzliche Dependency — nutzt das uuid-Crate aus dem Core-Baum.
fn uuid_v4_string() -> String {
    // Über den Store-Weg wäre es ein Add; hier reicht ein zufälliger Bezeichner
    // für Saved-Search-IDs. std bietet keine UUIDs — wir bauen eine v4-ähnliche
    // ID aus Zufallsbytes der Systemzeit + Adressen-Entropie wäre schwach.
    // Deshalb: uuid-Crate (bereits transitive Dependency).
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_capture_and_visible_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let mut state = AppState {
            store: Some(Arc::new(store)),
            init_error: None,
            tasks: vec![],
            visible: vec![],
            filter: SidebarFilter::Inbox,
            search_query: String::new(),
            sort: SortOrder::Id,
            sort_ascending: true,
            settings: Settings::default(),
        };
        state
            .quick_capture("Einkaufen gehen +haushalt project:privat due:tomorrow priority:H")
            .unwrap();
        assert_eq!(state.tasks.len(), 1);
        let t = &state.tasks[0];
        assert_eq!(t.description, "Einkaufen gehen");
        assert_eq!(t.project.as_deref(), Some("privat"));
        assert_eq!(t.priority.as_deref(), Some("H"));
        assert!(t.due.is_some());

        // Task hat Projekt+Tag → nicht im Eingang.
        assert!(state.visible.is_empty());
        state.filter = SidebarFilter::Todo;
        state.rebuild_visible();
        assert_eq!(state.visible.len(), 1);

        // Counts-JSON enthält die erwarteten Schlüssel.
        let counts: serde_json::Value = serde_json::from_str(&state.counts_json()).unwrap();
        assert_eq!(counts["todo"], 1);
        assert_eq!(counts["inbox"], 0);
    }

    #[test]
    fn mark_done_smart_creates_followup_for_recurring() {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let mut state = AppState {
            store: Some(Arc::new(store)),
            init_error: None,
            tasks: vec![],
            visible: vec![],
            filter: SidebarFilter::Todo,
            search_query: String::new(),
            sort: SortOrder::Id,
            sort_ascending: true,
            settings: Settings::default(),
        };
        let store = state.store.clone().unwrap();
        let uuid = store
            .add_task_full("Gießen".into(), None, vec![], Some(1_800_000_000))
            .unwrap();
        store.set_recur(uuid.clone(), Some("daily".into())).unwrap();
        state.refresh().unwrap();

        state.mark_done_smart(&uuid).unwrap();
        // Alte Instanz erledigt, neue Pending-Instanz mit due+1d existiert.
        let pending: Vec<_> = state
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect();
        assert_eq!(pending.len(), 1);
        assert_ne!(pending[0].uuid, uuid);
        assert_eq!(pending[0].recur.as_deref(), Some("daily"));
        assert!(pending[0].due.unwrap() > 1_800_000_000);
    }

    #[test]
    fn rename_and_clear_tag_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let mut state = AppState {
            store: Some(Arc::new(store)),
            init_error: None,
            tasks: vec![],
            visible: vec![],
            filter: SidebarFilter::Todo,
            search_query: String::new(),
            sort: SortOrder::Id,
            sort_ascending: true,
            settings: Settings::default(),
        };
        let store = state.store.clone().unwrap();
        store
            .add_task_full(
                "A".into(),
                Some("alt".into()),
                vec!["x".into()],
                None,
            )
            .unwrap();
        store
            .add_task_full(
                "B".into(),
                Some("alt".into()),
                vec!["x".into(), "y".into()],
                None,
            )
            .unwrap();
        state.refresh().unwrap();

        state.rename_project("alt", "neu").unwrap();
        assert!(state.tasks.iter().all(|t| t.project.as_deref() == Some("neu")));

        state.rename_tag("x", "z").unwrap();
        assert!(state.tasks.iter().all(|t| t.tags.contains(&"z".to_string())));
        assert!(state.tasks.iter().all(|t| !t.tags.contains(&"x".to_string())));

        state.clear_tag("z").unwrap();
        assert!(state.tasks.iter().all(|t| !t.tags.contains(&"z".to_string())));

        state.clear_project("neu").unwrap();
        assert!(state.tasks.iter().all(|t| t.project.is_none()));
    }

    #[test]
    fn projects_json_builds_hierarchy_with_implicit_parents() {
        let mut state = AppState {
            store: None,
            init_error: None,
            tasks: vec![],
            visible: vec![],
            filter: SidebarFilter::Todo,
            search_query: String::new(),
            sort: SortOrder::Id,
            sort_ascending: true,
            settings: Settings::default(),
        };
        let a = TaskInfo {
            uuid: "a".into(),
            description: "A".into(),
            project: Some("Work.Sub.Deep".into()),
            tags: vec![],
            due: None,
            status: TaskStatus::Pending,
            entry: None,
            working_set_id: Some(1),
            priority: None,
            annotations: vec![],
            wait: None,
            recur: None,
            scheduled: None,
            depends: vec![],
            is_blocked: false,
            is_blocking: false,
        };
        let mut b = a.clone();
        b.uuid = "b".into();
        b.project = Some("Workshop".into());
        state.tasks = vec![a.clone(), b];

        let parsed: serde_json::Value =
            serde_json::from_str(&state.projects_json()).unwrap();
        let items = parsed.as_array().unwrap();
        let names: Vec<&str> = items.iter().map(|i| i["name"].as_str().unwrap()).collect();
        // Implizite Eltern vorhanden, hierarchische Reihenfolge, Workshop getrennt.
        assert_eq!(names, vec!["Work", "Work.Sub", "Work.Sub.Deep", "Workshop"]);
        assert_eq!(items[0]["depth"], 0);
        assert_eq!(items[0]["hasChildren"], true);
        // Präfix-Zählung: implizites Elternprojekt zählt das tiefe Kind mit,
        // Workshop wird nicht von "Work" mitgezählt.
        assert_eq!(items[0]["count"], 1);
        assert_eq!(items[2]["depth"], 2);
        assert_eq!(items[2]["label"], "Deep");
        assert_eq!(items[3]["count"], 1);
    }

    #[test]
    fn saved_search_crud_and_duplicate_detection() {
        let mut state = AppState {
            store: None,
            init_error: None,
            tasks: vec![],
            visible: vec![],
            filter: SidebarFilter::Inbox,
            search_query: String::new(),
            sort: SortOrder::Id,
            sort_ascending: true,
            settings: Settings::default(),
        };
        // Persistenz schlägt in Test-Umgebung ggf. ins echte Config-Verzeichnis —
        // dafür sorgt XDG_CONFIG_HOME im Test-Runner; hier nur Logik prüfen.
        let id = match state.save_search("Büro offen", "projekt:büro status:offen") {
            Ok(id) => id,
            Err(_) => return, // Ohne schreibbares Config-Verzeichnis nichts zu prüfen.
        };
        assert!(state.save_search("büro OFFEN", "x").is_err(), "Duplikat muss abgelehnt werden");
        assert_eq!(
            state.saved_search_query(&id).as_deref(),
            Some("projekt:büro status:offen")
        );
        state.rename_saved_search(&id, "Arbeit").unwrap();
        state.delete_saved_search(&id).unwrap();
        assert!(state.settings.saved_searches.is_empty());
    }
}
