//! vergissmeinnicht-core — taskchampion-Wrapper des KDE-Ports.
//!
//! Portiert aus der macOS-Version (dort via UniFFI an Swift exportiert); hier eine
//! reine Rust-Bibliothek, die von der cxx-qt-Bridge der App konsumiert wird. Die
//! öffentliche API (TaskStore-Methoden, TaskInfo, VmError) ist bewusst 1:1 zur
//! macOS-Version gehalten, damit Fixes zwischen beiden Ports wandern können.

use std::sync::{Mutex, MutexGuard};

use std::str::FromStr;

// Re-Export für die App: chrono kommt aus taskchampion, damit App und Core
// garantiert dieselbe Version verwenden.
pub use taskchampion::chrono;

use taskchampion::{
    chrono::{DateTime, Utc},
    storage::AccessMode,
    Annotation, Operation, Operations, Replica, ServerConfig, SqliteStorage, Status, Tag,
};
use uuid::Uuid;

// Typ-Alias auf die konkrete Replica-Variante (Konvention aus der macOS-Version).
type AppReplica = Replica<SqliteStorage>;

// ─── Property-Key-Konstanten ─────────────────────────────────────────────────

const PROP_PROJECT: &str = "project";
const PROP_PRIORITY: &str = "priority";
const PROP_RECUR: &str = "recur";
const PROP_SCHEDULED: &str = "scheduled";

/// Startet einen Mutations-Batch mit einem UndoPoint — Grundlage für
/// `undo_last_change`; ein Batch = ein rückgängig machbarer Schritt.
fn mutation_ops() -> Operations {
    vec![Operation::UndoPoint]
}

// ─── Smoketest ──────────────────────────────────────────────────────────────

pub fn ping() -> String {
    "pong".to_string()
}

// ─── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("Storage: {msg}")]
    Storage { msg: String },
    #[error("Conversion: {msg}")]
    Conversion { msg: String },
    #[error("Not found: {uuid}")]
    NotFound { uuid: String },
    #[error("Sync: {msg}")]
    Sync { msg: String },
    #[error("Internal: {msg}")]
    Internal { msg: String },
}

impl From<taskchampion::Error> for VmError {
    fn from(e: taskchampion::Error) -> Self {
        match e {
            taskchampion::Error::Database(s) => Self::Storage { msg: s },
            taskchampion::Error::Usage(s) => Self::Internal { msg: s },
            taskchampion::Error::Server(s) => Self::Sync { msg: s },
            other => Self::Internal { msg: other.to_string() },
        }
    }
}

fn parse_uuid(uuid: &str) -> Result<Uuid, VmError> {
    Uuid::parse_str(uuid).map_err(|e| VmError::Conversion { msg: e.to_string() })
}

/// Konvertiert Unix-Sekunden in einen `DateTime<Utc>`. Out-of-range-Werte werden als
/// Conversion-Error gemeldet, statt zu panicen.
fn timestamp_from_secs(secs: i64) -> Result<DateTime<Utc>, VmError> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| VmError::Conversion { msg: format!("due timestamp out of range: {secs}") })
}

/// Iteriert über die User-Tags eines Tasks (filtert synthetische TaskChampion-Tags
/// wie PENDING, OVERDUE etc. heraus).
fn user_tags(task: &taskchampion::Task) -> impl Iterator<Item = Tag> + '_ {
    task.get_tags().filter(|t| t.is_user())
}

/// Validiert die Sync-Server-URL: Schema muss `http` oder `https` sein, und der
/// Host-Teil darf nicht leer sein. Kein `url`-Crate nötig — einfache String-Prüfung
/// reicht, um den häufigsten Konfigurationsfehler (fehlendes Schema) früh abzufangen.
fn validate_sync_url(url: &str) -> Result<(), VmError> {
    // Schema extrahieren: alles vor dem ersten "://"
    let after_schema = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| VmError::Sync {
            msg: format!("server_url must start with http:// or https://, got: {url:?}"),
        })?;

    // Host ist alles vor dem ersten '/' (oder der gesamte Rest).
    let host = after_schema.split('/').next().unwrap_or("");
    // Entferne Port-Angabe (host:port), sodass nur der Hostname übrig bleibt.
    let hostname = host.split(':').next().unwrap_or("");
    if hostname.is_empty() {
        return Err(VmError::Sync {
            msg: format!("server_url has no host: {url:?}"),
        });
    }

    Ok(())
}

/// Baut ein `TaskInfo` aus einem `taskchampion::Task` plus optionaler Working-Set-ID.
/// Zentralisiert die Property-Extraktion (project, tags, due, entry, priority, annotations).
///
/// `is_blocked`/`is_blocking` werden NICHT aus dem Task allein abgeleitet — sie hängen vom
/// Abhängigkeits-*Graphen* (welcher Task von welchem) ab und kommen aus der einmal pro
/// `list_tasks`-Lauf berechneten `dependency_map` herein.
fn build_task_info(
    task: &taskchampion::Task,
    uuid: Uuid,
    working_set_id: Option<u32>,
    is_blocked: bool,
    is_blocking: bool,
) -> TaskInfo {
    let project = task
        .get_value(PROP_PROJECT)
        .map(|s| s.to_owned())
        .filter(|s| !s.is_empty());
    let tags: Vec<String> = user_tags(task).map(|t| t.to_string()).collect();
    let due = task.get_due().map(|ts| ts.timestamp());
    let entry = task.get_entry().map(|ts| ts.timestamp());
    let priority = Some(task.get_priority().to_owned()).filter(|p| !p.is_empty());
    let annotations: Vec<AnnotationInfo> = task
        .get_annotations()
        .map(|a| AnnotationInfo {
            entry: a.entry.timestamp(),
            description: a.description,
        })
        .collect();
    let wait = task.get_wait().map(|ts| ts.timestamp());
    let recur = task
        .get_value(PROP_RECUR)
        .map(|s| s.to_owned())
        .filter(|s| !s.is_empty());
    let scheduled = task
        .get_value(PROP_SCHEDULED)
        .and_then(|s| s.parse::<i64>().ok());
    // `depends` = ALLE Abhängigkeiten als UUID-Strings, unabhängig vom Status (kann laut
    // taskchampion-Doku auch nicht (mehr) existierende UUIDs enthalten). Speist den
    // Detail-Editor — bewusst getrennt von den pending-only `is_blocked`/`is_blocking`.
    let depends: Vec<String> = task.get_dependencies().map(|u| u.to_string()).collect();
    // Von der Taskwarrior-CLI generierte Instanz eines Recurring-Templates
    // (trägt `parent`/`imask`) — die App darf für solche Tasks NIE eigene
    // Folge-Instanzen erzeugen, sonst entstehen Duplikate neben der CLI-Engine.
    let is_recurring_child =
        task.get_value("parent").is_some() || task.get_value("imask").is_some();
    let start = task.get_value("start").and_then(|s| s.parse::<i64>().ok());
    let until = task.get_value("until").and_then(|s| s.parse::<i64>().ok());
    let modified = task.get_modified().map(|ts| ts.timestamp());
    // Fremd-/UDA-Properties für die read-only-Anzeige: alles, was weder ein
    // typisiertes Attribut noch App-bekannt noch Recurrence-/Mirror-Buchhaltung
    // ist. get_udas ist deprecated, aber die einzige öffentliche Roh-Iteration
    // am Task (die Alternative wäre ein zweiter Storage-Zugriff via TaskData).
    #[allow(deprecated)]
    let udas: Vec<(String, String)> = task
        .get_udas()
        .filter(|((ns, key), _)| {
            let flat = if ns.is_empty() { (*key).to_string() } else { format!("{ns}.{key}") };
            !matches!(
                flat.as_str(),
                "project" | "recur" | "scheduled" | "until" | "rtype" | "mask" | "parent"
                    | "imask" | "tags" | "depends"
            )
        })
        .map(|((ns, key), v)| {
            let flat = if ns.is_empty() { key.to_string() } else { format!("{ns}.{key}") };
            (flat, v.to_string())
        })
        .collect();
    let status = match task.get_status() {
        Status::Pending => TaskStatus::Pending,
        Status::Completed => TaskStatus::Completed,
        Status::Deleted => TaskStatus::Deleted,
        Status::Recurring => TaskStatus::Recurring,
        // Unknown-Status (zukünftige taskchampion-Erweiterungen) konservativ als Deleted —
        // weniger schlimm als zu zeigen.
        Status::Unknown(_) => TaskStatus::Deleted,
    };
    TaskInfo {
        uuid: uuid.to_string(),
        description: task.get_description().to_owned(),
        project,
        tags,
        due,
        status,
        entry,
        working_set_id,
        priority,
        annotations,
        wait,
        recur,
        scheduled,
        depends,
        is_blocked,
        is_blocking,
        is_recurring_child,
        start,
        until,
        modified,
        udas,
    }
}

// ─── Records ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Completed,
    Deleted,
    Recurring,
}

#[derive(Debug, Clone)]
pub struct AnnotationInfo {
    /// Entry-Zeitpunkt der Annotation als Unix-Sekunden (i64). Dient gleichzeitig als
    /// Schlüssel beim Entfernen.
    pub entry: i64,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub uuid: String,
    pub description: String,
    /// Wert der `project`-Property; leer falls nicht gesetzt.
    pub project: Option<String>,
    /// User-Tags (synthetische TaskChampion-Tags wie PENDING/OVERDUE sind herausgefiltert).
    pub tags: Vec<String>,
    /// Due-Date als Unix-Sekunden (i64). Der i64-Pfad ist an der QML-Grenze robust
    /// (QML sieht ein `double`-Datum via `new Date(secs * 1000)`).
    pub due: Option<i64>,
    /// Status des Tasks (pending / completed / deleted / recurring).
    pub status: TaskStatus,
    /// Entry-Zeitpunkt der Task (Anlage-Datum) als Unix-Sekunden. Wird beim Anlegen gesetzt.
    pub entry: Option<i64>,
    /// Working-Set-ID (Taskwarrior-typische numerische ID, 1-N). Nur für Pending-Tasks
    /// definiert; für Completed/Deleted ist `None`. Kein stabiler Identifier (das ist
    /// die UUID); Rationale zu u32 und Lifecycle: siehe docs/architecture.md.
    pub working_set_id: Option<u32>,
    /// Priority-Property als Rohwert (typisch `H` / `M` / `L`); nicht validiert.
    pub priority: Option<String>,
    /// Annotations zum Task, in beliebiger Reihenfolge.
    pub annotations: Vec<AnnotationInfo>,
    /// Wait-Property (Snooze) als Unix-Sekunden. Liegt ein Wert in der Zukunft,
    /// gilt der Task als „wartend" — Taskwarrior versteckt solche Tasks per Default
    /// aus `task list`; die App zeigt sie in einer eigenen Sidebar-Sektion.
    pub wait: Option<i64>,
    /// Recur-Property als Rohstring (z.B. `daily`, `weekly`, `monthly`, `1d`, `2w`).
    /// Wir interpretieren es App-seitig — TaskChampion-Lib generiert keine Children.
    pub recur: Option<String>,
    /// Scheduled-Property (Start-Datum / Defer-Until) als Unix-Sekunden. Tasks mit
    /// `scheduled` in der Zukunft sind „geplant" und werden aus ToDo/Eingang/Überfällig
    /// ausgeblendet, bis das Datum erreicht ist.
    pub scheduled: Option<i64>,
    /// UUID-Strings aller Tasks, von denen dieser Task abhängt (`depends`), unabhängig
    /// vom Status. Native Taskwarrior-Relation. Speist den Detail-Editor.
    pub depends: Vec<String>,
    /// `true`, wenn dieser Task von mindestens einem noch *pending* Task abhängt
    /// (Taskwarrior `+BLOCKED`). Aus `Replica::dependency_map()` abgeleitet, nicht aus
    /// dem Task allein — daher ein abgeleitetes Feld analog `working_set_id`.
    pub is_blocked: bool,
    /// `true`, wenn mindestens ein anderer noch *pending* Task von diesem abhängt
    /// (Taskwarrior `+BLOCKING`).
    pub is_blocking: bool,
    /// CLI-generierte Instanz eines Recurring-Templates (`parent`/`imask` gesetzt);
    /// die App erzeugt für solche Tasks keine eigenen Folge-Instanzen.
    pub is_recurring_child: bool,
    /// `start`-Zeitpunkt (Unix-Sekunden); gesetzt = Task ist aktiv (`task start`).
    pub start: Option<i64>,
    /// `until`-Ablaufdatum; die CLI löscht den Task still, sobald es verstreicht.
    pub until: Option<i64>,
    /// Letzte Änderung (Unix-Sekunden).
    pub modified: Option<i64>,
    /// Fremd-/UDA-Properties (Key → Rohwert) für die read-only-Anzeige.
    pub udas: Vec<(String, String)>,
}

/// Roh-Repräsentation eines Tasks: (uuid, Property-Paare).
pub type RawTask = (String, Vec<(String, String)>);

// ─── TaskStore ──────────────────────────────────────────────────────────────

pub struct TaskStore {
    replica: Mutex<AppReplica>,
    rt: tokio::runtime::Runtime,
}

impl TaskStore {
    /// Lockt die Replica-Mutex. Zentralisiert den wiederkehrenden Boilerplate.
    ///
    /// SICHERHEITSHINWEIS: Der MutexGuard wird über den gesamten `rt.block_on`-Aufruf
    /// gehalten. Die current-thread-Runtime (Feature `rt`, kein `rt-multi-thread`) ist
    /// dafür Voraussetzung. Das bewusste Halten des Guards über `block_on` ist
    /// intentional — kein anderer Thread kann die Replica konkurrierend verwenden.
    ///
    /// Bei einem poisoned Mutex (eine `block_on`-Operation paniced, während der Guard
    /// gehalten wurde) wird der innere Guard via `into_inner()` geborgen, statt jeden
    /// weiteren Call bis zum App-Neustart abzuweisen. Zulässig, weil der Zugriff
    /// streng über diesen einen Mutex serialisiert ist: es gibt keinen konkurrierenden
    /// Leser, der inkonsistenten In-Memory-State sieht, und die SQLite-Daten auf
    /// Platte sind transaktional/durabel.
    fn lock_replica(&self) -> Result<MutexGuard<'_, AppReplica>, VmError> {
        Ok(self.replica.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl TaskStore {
    /// Öffnet (oder legt an) eine TaskChampion-SQLite-Replica unter `db_path`.
    /// Der Pfad muss ein Verzeichnis sein (TaskChampion legt darin SQLite-Dateien an).
    pub fn new(db_path: String) -> Result<Self, VmError> {
        // current-thread runtime — passt zu features = ["rt", "macros", "sync"]
        // (Runtime::new() würde rt-multi-thread voraussetzen).
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| VmError::Internal { msg: e.to_string() })?;

        let replica = rt.block_on(async {
            let storage =
                SqliteStorage::new(std::path::PathBuf::from(db_path), AccessMode::ReadWrite, true)
                    .await?;
            Ok::<_, taskchampion::Error>(Replica::new(storage))
        })?;

        Ok(Self {
            replica: Mutex::new(replica),
            rt,
        })
    }

    /// Legt einen neuen Task mit der gegebenen Description an und gibt seine UUID zurück.
    pub fn add_task(&self, description: String) -> Result<String, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let uuid = self.rt.block_on(async {
            let new_uuid = Uuid::new_v4();
            let mut ops = mutation_ops();
            let mut task = replica.create_task(new_uuid, &mut ops).await?;
            task.set_description(description, &mut ops)?;
            task.set_status(Status::Pending, &mut ops)?;
            task.set_entry(Some(Utc::now()), &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, taskchampion::Error>(new_uuid)
        })?;

        Ok(uuid.to_string())
    }

    /// Legt einen neuen Task mit voller Metadaten-Persistierung an: project (raw value),
    /// User-Tags und due (Unix-Sekunden). Leere Tags und None/Empty-Project werden
    /// nicht geschrieben. Tag-Strings müssen TaskChampion-konform sein
    /// (kein Whitespace, kein Operator-Zeichen am Anfang, kein Doppelpunkt darin).
    pub fn add_task_full(
        &self,
        description: String,
        project: Option<String>,
        tags: Vec<String>,
        due: Option<i64>,
    ) -> Result<String, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let due_ts = match due {
            Some(secs) => Some(timestamp_from_secs(secs)?),
            None => None,
        };

        let uuid = self.rt.block_on(async {
            let new_uuid = Uuid::new_v4();
            let mut ops = mutation_ops();
            let mut task = replica.create_task(new_uuid, &mut ops).await?;
            task.set_description(description, &mut ops)?;
            task.set_status(Status::Pending, &mut ops)?;
            task.set_entry(Some(Utc::now()), &mut ops)?;

            if let Some(p) = project.as_ref().filter(|s| !s.is_empty()) {
                task.set_value(PROP_PROJECT, Some(p.clone()), &mut ops)?;
            }
            for tag_str in &tags {
                let tag = Tag::from_str(tag_str)
                    .map_err(|e| VmError::Conversion { msg: format!("invalid tag {tag_str:?}: {e}") })?;
                task.add_tag(&tag, &mut ops)?;
            }
            if let Some(ts) = due_ts {
                task.set_due(Some(ts), &mut ops)?;
            }

            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(new_uuid)
        })?;

        Ok(uuid.to_string())
    }

    /// Aktualisiert Metadaten eines bestehenden Tasks in einer einzigen Commit-Batch:
    /// Description, project (None = clear), Tags (komplette Ersetzung), due (None = clear).
    pub fn update_task_metadata(
        &self,
        uuid: String,
        description: String,
        project: Option<String>,
        tags: Vec<String>,
        due: Option<i64>,
    ) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let due_ts = match due {
            Some(secs) => Some(timestamp_from_secs(secs)?),
            None => None,
        };

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;

            task.set_description(description, &mut ops)?;

            // Project: explizit setzen oder clearen.
            let project_value = project.filter(|s| !s.is_empty());
            task.set_value(PROP_PROJECT, project_value, &mut ops)?;

            // Tags: User-Tags vorher entfernen, dann neue setzen (Synthetics bleiben).
            let current_user_tags: Vec<Tag> = user_tags(&task).collect();
            for t in &current_user_tags {
                task.remove_tag(t, &mut ops)?;
            }
            for tag_str in &tags {
                let tag = Tag::from_str(tag_str)
                    .map_err(|e| VmError::Conversion { msg: format!("invalid tag {tag_str:?}: {e}") })?;
                task.add_tag(&tag, &mut ops)?;
            }

            task.set_due(due_ts, &mut ops)?;

            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Listet alle Tasks (Pending oder optional auch Completed/Recurring).
    /// Deleted-Tasks bleiben immer aussen vor.
    ///
    /// Bei `include_completed = false` wird nur das Working Set durchlaufen — das enthält
    /// ausschließlich Pending-Tasks. Recurring-Master und Completed bleiben in diesem
    /// Modus unsichtbar; wer sie braucht, muss `include_completed = true` setzen.
    /// Bei `true` werden Pending mit `working_set_id` zuerst ausgegeben, danach alle
    /// übrigen sichtbaren Status (Completed, Recurring, sowie ggf. Pending ohne
    /// Working-Set-Eintrag) — die App sortiert clientseitig.
    pub fn list_tasks(&self, include_completed: bool) -> Result<Vec<TaskInfo>, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let infos = self.rt.block_on(async {
            // Abhängigkeits-Graph einmal pro Lauf berechnen (NICHT per Task — `dependency_map`
            // scannt das ganze Working Set). `force = false` genügt: `commit_operations`
            // invalidiert den Cache nach jeder Mutation, dieser Read-Pfad sieht also stets
            // einen frischen Graphen. `dependencies(u)` = Tasks, von denen u abhängt → u ist
            // BLOCKED; `dependents(u)` = Tasks, die von u abhängen → u ist BLOCKING.
            // Beide pending-only (entspricht +BLOCKED/+BLOCKING).
            let depmap = replica.dependency_map(false).await?;
            let blocked = |uuid: Uuid| depmap.dependencies(uuid).next().is_some();
            let blocking = |uuid: Uuid| depmap.dependents(uuid).next().is_some();

            let ws = replica.working_set().await?;
            let mut out = Vec::new();
            let mut seen_pending = std::collections::HashSet::new();

            for (index, uuid) in ws.iter() {
                if let Some(task) = replica.get_task(uuid).await? {
                    if task.get_status() == Status::Pending {
                        seen_pending.insert(uuid);
                        out.push(build_task_info(&task, uuid, Some(index as u32), blocked(uuid), blocking(uuid)));
                    }
                }
            }

            if include_completed {
                let all = replica.all_tasks().await?;
                let mut entries: Vec<_> = all.iter().collect();
                entries.sort_by_key(|(uuid, _)| *uuid); // deterministisch über App-Starts
                for (uuid, task) in entries {
                    match task.get_status() {
                        Status::Completed => out.push(build_task_info(task, *uuid, None, blocked(*uuid), blocking(*uuid))),
                        // Recurring-Master haben keinen Working-Set-Eintrag und sind
                        // ausschließlich über diesen Pfad sichtbar (siehe Doc oben).
                        Status::Recurring => out.push(build_task_info(task, *uuid, None, blocked(*uuid), blocking(*uuid))),
                        // Pending ohne Working-Set-Eintrag (sollte nicht vorkommen,
                        // aber theoretisch möglich) — sicherheitshalber ergänzen.
                        Status::Pending if !seen_pending.contains(uuid) => {
                            out.push(build_task_info(task, *uuid, None, blocked(*uuid), blocking(*uuid)));
                        }
                        _ => {}
                    }
                }
            }

            Ok::<_, taskchampion::Error>(out)
        })?;

        Ok(infos)
    }

    /// Backwards-Compat-Alias: nur Pending, mit Working-Set-IDs.
    pub fn list_pending(&self) -> Result<Vec<TaskInfo>, VmError> {
        self.list_tasks(false)
    }

    /// Gibt die Anzahl der lokalen Operationen zurück, die noch nicht mit dem Server
    /// synchronisiert wurden. Wird vom Sync-Indikator der App genutzt.
    pub fn num_local_operations(&self) -> Result<u64, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let count = self.rt.block_on(async {
            replica.num_local_operations().await
        })?;

        Ok(count as u64)
    }

    /// Anzahl der rückgängig machbaren Schritte (UndoPoints) seit dem letzten Sync.
    pub fn num_undo_points(&self) -> Result<u64, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let count = self.rt.block_on(async { replica.num_undo_points().await })?;

        Ok(count as u64)
    }

    /// Macht den jüngsten Mutations-Batch rückgängig (bis zum letzten UndoPoint).
    /// Gibt `false` zurück, wenn es nichts rückgängig zu machen gab oder der
    /// Zustand sich inzwischen geändert hat (taskchampion lehnt dann ab).
    pub fn undo_last_change(&self) -> Result<bool, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let undone = self.rt.block_on(async {
            let undo_ops = replica.get_undo_operations().await?;
            if undo_ops.is_empty() {
                return Ok::<_, taskchampion::Error>(false);
            }
            replica.commit_reversed_operations(undo_ops).await
        })?;

        Ok(undone)
    }

    /// Markiert die Task mit `uuid` als erledigt (`Status::Completed`) und committet.
    pub fn mark_done(&self, uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_status(Status::Completed, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Ändert die Beschreibung der Task mit `uuid` und committet.
    pub fn modify_description(
        &self,
        uuid: String,
        new_description: String,
    ) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_description(new_description, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Markiert die Task mit `uuid` als gelöscht (`Status::Deleted`) und committet.
    /// Das Operations-Log bleibt erhalten — kein Purge.
    pub fn delete_task(&self, uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_status(Status::Deleted, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Hängt eine Annotation an die Task mit `uuid` an. Entry-Zeitstempel = `Utc::now()`.
    pub fn add_annotation(&self, uuid: String, annotation: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.add_annotation(
                Annotation {
                    entry: Utc::now(),
                    description: annotation,
                },
                &mut ops,
            )?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Entfernt die Annotation mit dem gegebenen Entry-Zeitstempel (Unix-Sekunden).
    /// Wird vom Detail-Editor zum Löschen einzelner Annotations genutzt.
    pub fn remove_annotation(&self, uuid: String, entry: i64) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let ts = timestamp_from_secs(entry)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.remove_annotation(ts, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt das `project`-Property. `None` oder leerer String entfernt es.
    pub fn set_project(&self, uuid: String, project: Option<String>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            let value = project.filter(|s| !s.is_empty());
            task.set_value(PROP_PROJECT, value, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt das `due`-Property (Unix-Sekunden). `None` entfernt die Fälligkeit.
    pub fn set_due(&self, uuid: String, due: Option<i64>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let due_ts = match due {
            Some(secs) => Some(timestamp_from_secs(secs)?),
            None => None,
        };
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_due(due_ts, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Markiert eine Pending-Task als erledigt und legt — sofern `recur` und
    /// `new_due` gesetzt sind — in derselben Operations-Batch eine neue Pending-
    /// Instanz an. Description/Project/Tags/Priority werden kopiert; Annotations
    /// werden bewusst NICHT übertragen, da Annotations zeitpunktbezogen sind.
    /// Gibt die UUID der neu erzeugten Folge-Instanz zurück (`None`, wenn keine).
    #[allow(clippy::too_many_arguments)]
    pub fn mark_done_with_followup(
        &self,
        uuid: String,
        new_due: Option<i64>,
        recur: Option<String>,
        priority: Option<String>,
        project: Option<String>,
        tags: Vec<String>,
        description: String,
    ) -> Result<Option<String>, VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let due_ts = match new_due {
            Some(secs) => Some(timestamp_from_secs(secs)?),
            None => None,
        };
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        let new_uuid: Option<Uuid> = self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_status(Status::Completed, &mut ops)?;

            // Folge-Instanz nur, wenn recur UND new_due gesetzt.
            let create_followup = recur.as_ref().map(|s| !s.is_empty()).unwrap_or(false) && due_ts.is_some();
            let new_uuid = if create_followup {
                let new_uuid = Uuid::new_v4();
                let mut new_task = replica.create_task(new_uuid, &mut ops).await?;
                new_task.set_description(description, &mut ops)?;
                new_task.set_status(Status::Pending, &mut ops)?;
                new_task.set_entry(Some(Utc::now()), &mut ops)?;
                if let Some(p) = project.as_ref().filter(|s| !s.is_empty()) {
                    new_task.set_value(PROP_PROJECT, Some(p.clone()), &mut ops)?;
                }
                for tag_str in &tags {
                    let tag = Tag::from_str(tag_str)
                        .map_err(|e| VmError::Conversion { msg: format!("invalid tag {tag_str:?}: {e}") })?;
                    new_task.add_tag(&tag, &mut ops)?;
                }
                if let Some(ts) = due_ts {
                    new_task.set_due(Some(ts), &mut ops)?;
                }
                if let Some(p) = priority.as_ref().filter(|s| !s.is_empty()) {
                    new_task.set_value(PROP_PRIORITY, Some(p.clone()), &mut ops)?;
                }
                if let Some(r) = recur.as_ref().filter(|s| !s.is_empty()) {
                    new_task.set_value(PROP_RECUR, Some(r.clone()), &mut ops)?;
                }
                Some(new_uuid)
            } else {
                None
            };

            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(new_uuid)
        })?;

        Ok(new_uuid.map(|u| u.to_string()))
    }

    /// Setzt das `scheduled`-Property (Start-Datum als Unix-Sekunden). `None`
    /// entfernt es. Tasks mit `scheduled` in der Zukunft gelten App-seitig als
    /// „geplant".
    pub fn set_scheduled(&self, uuid: String, scheduled: Option<i64>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            // taskchampion speichert `scheduled` als String (Unix-Sekunden, Dezimalzahl).
            // `set_value` erwartet `Option<String>` — daher i64 → String-Konversion.
            let value = scheduled.map(|s| s.to_string());
            task.set_value(PROP_SCHEDULED, value, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt das `recur`-Property (z.B. `daily`, `weekly`, `1d`, `2w`). `None` oder
    /// leerer String entfernen es. Wird app-seitig interpretiert — TaskChampion-Lib
    /// generiert keine Children automatisch.
    pub fn set_recur(&self, uuid: String, recur: Option<String>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            let value = recur.filter(|s| !s.is_empty());
            task.set_value(PROP_RECUR, value, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt das `priority`-Property (`H` / `M` / `L`). `None` oder leerer String
    /// entfernt es. Es wird nicht validiert — Taskwarrior toleriert beliebige
    /// Strings, sortiert aber clientseitig nach diesem Wert.
    pub fn set_priority(&self, uuid: String, priority: Option<String>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            let value = priority.filter(|s| !s.is_empty());
            task.set_value(PROP_PRIORITY, value, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt eine rohe Task-Property (z. B. `until`, UDAs; in Tests auch
    /// `parent`/`status`). Leerer Wert entfernt die Property. NICHT für
    /// Properties mit typisierten Settern (description, due, wait …) verwenden —
    /// die verwalten Nebeneffekte wie `end` selbst.
    pub fn set_raw_property(
        &self,
        uuid: String,
        key: String,
        value: Option<String>,
    ) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            let value = value.filter(|s| !s.is_empty());
            task.set_value(key, value, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Startet die Task (`task start`): setzt `start` auf jetzt, falls nicht aktiv.
    pub fn start_task(&self, uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.start(&mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Stoppt die Task (`task stop`): entfernt `start`.
    pub fn stop_task(&self, uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.stop(&mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt/entfernt das `until`-Ablaufdatum (Unix-Sekunden, CLI-Format).
    pub fn set_until(&self, uuid: String, until: Option<i64>) -> Result<(), VmError> {
        self.set_raw_property(uuid, "until".into(), until.map(|s| s.to_string()))
    }

    /// Roh-Dump aller Tasks (uuid → Property-Paare) für den JSON-Export der App.
    /// Enthält ALLE Properties inkl. UDAs und Recurrence-Buchhaltung.
    pub fn all_raw_tasks(&self) -> Result<Vec<RawTask>, VmError> {
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let all = replica.all_task_data().await?;
            let mut result = Vec::with_capacity(all.len());
            for (uuid, data) in all {
                let props: Vec<(String, String)> = data
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                result.push((uuid.to_string(), props));
            }
            Ok::<_, VmError>(result)
        })
    }

    /// Fügt einen einzelnen User-Tag hinzu. No-op, falls der Tag bereits existiert.
    pub fn add_tag(&self, uuid: String, tag: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let tag_obj = Tag::from_str(&tag)
            .map_err(|e| VmError::Conversion { msg: format!("invalid tag {tag:?}: {e}") })?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            if !task.has_tag(&tag_obj) {
                task.add_tag(&tag_obj, &mut ops)?;
                replica.commit_operations(ops).await?;
            }
            Ok::<_, VmError>(())
        })
    }

    /// Entfernt einen User-Tag. No-op, falls der Tag nicht existiert.
    pub fn remove_tag(&self, uuid: String, tag: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let tag_obj = Tag::from_str(&tag)
            .map_err(|e| VmError::Conversion { msg: format!("invalid tag {tag:?}: {e}") })?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            if task.has_tag(&tag_obj) {
                task.remove_tag(&tag_obj, &mut ops)?;
                replica.commit_operations(ops).await?;
            }
            Ok::<_, VmError>(())
        })
    }

    /// Fügt eine Abhängigkeit hinzu: `uuid` hängt fortan von `depends_on_uuid` ab
    /// (native Taskwarrior `depends`). Idempotent — `add_dependency` setzt nur das
    /// `dep_<uuid>`-Property, ein erneuter Aufruf ist ein No-op. Es wird nicht geprüft,
    /// ob das Ziel existiert oder ob ein Zyklus entsteht — Taskwarrior selbst erzwingt
    /// das ebenfalls nicht.
    pub fn add_dependency(&self, uuid: String, depends_on_uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let dep_uuid = parse_uuid(&depends_on_uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.add_dependency(dep_uuid, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Entfernt eine Abhängigkeit. Idempotent — `remove_dependency` löscht nur das
    /// `dep_<uuid>`-Property; existiert es nicht, ist der Aufruf ein No-op.
    pub fn remove_dependency(&self, uuid: String, depends_on_uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let dep_uuid = parse_uuid(&depends_on_uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.remove_dependency(dep_uuid, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Setzt das `wait`-Property (Unix-Sekunden). `None` entfernt es. Tasks mit
    /// `wait` in der Zukunft gelten als „wartend" (Snooze).
    pub fn set_wait(&self, uuid: String, wait: Option<i64>) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let wait_ts = match wait {
            Some(secs) => Some(timestamp_from_secs(secs)?),
            None => None,
        };
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_wait(wait_ts, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Reaktiviert einen Task: Status zurück auf Pending. Aufgerufen z.B., wenn
    /// User einen versehentlich erledigten Task wiederherstellen will.
    pub fn reactivate(&self, uuid: String) -> Result<(), VmError> {
        let task_uuid = parse_uuid(&uuid)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut ops = mutation_ops();
            let mut task = replica
                .get_task(task_uuid)
                .await?
                .ok_or(VmError::NotFound { uuid: uuid.clone() })?;
            task.set_status(Status::Pending, &mut ops)?;
            replica.commit_operations(ops).await?;
            Ok::<_, VmError>(())
        })
    }

    /// Synchronisiert die Replica gegen einen TaskChampion-Sync-Server.
    /// `client_id` muss ein UUID-String sein. `encryption_secret` wird als UTF-8-Bytes verwendet.
    ///
    /// `server_url` muss mit `http://` oder `https://` beginnen und einen nicht-leeren Host
    /// enthalten — sonst wird sofort `VmError::Sync` zurückgegeben, bevor taskchampion
    /// tief in der Netzwerk-Schicht einen weniger verständlichen Fehler produziert.
    pub fn sync(
        &self,
        server_url: String,
        client_id: String,
        encryption_secret: String,
    ) -> Result<(), VmError> {
        validate_sync_url(&server_url)?;

        let client_uuid = parse_uuid(&client_id)?;
        let mut guard = self.lock_replica()?;
        let replica: &mut AppReplica = &mut guard;

        self.rt.block_on(async {
            let mut server = ServerConfig::Remote {
                url: server_url,
                client_id: client_uuid,
                encryption_secret: encryption_secret.into_bytes(),
            }
            .into_server()
            .await?;
            replica.sync(&mut server, false).await?;
            Ok::<_, taskchampion::Error>(())
        })?;

        Ok(())
    }
}

// ─── Unit Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── timestamp_from_secs ──────────────────────────────────────────────────

    #[test]
    fn timestamp_from_secs_valid() {
        // Unix-Epoch selbst und ein repräsentativer Wert müssen fehlerfrei konvertieren.
        assert!(timestamp_from_secs(0).is_ok());
        assert!(timestamp_from_secs(1_700_000_000).is_ok());
    }

    #[test]
    fn timestamp_from_secs_out_of_range() {
        // chrono akzeptiert keine Werte außerhalb des i64-nanosekunden-Bereichs.
        let result = timestamp_from_secs(i64::MAX);
        assert!(matches!(result, Err(VmError::Conversion { .. })));
    }

    // ── parse_uuid ──────────────────────────────────────────────────────────

    #[test]
    fn parse_uuid_invalid_returns_conversion_error() {
        let result = parse_uuid("not-a-uuid");
        assert!(matches!(result, Err(VmError::Conversion { .. })));
    }

    #[test]
    fn parse_uuid_valid() {
        let result = parse_uuid("550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_ok());
    }

    // ── validate_sync_url ───────────────────────────────────────────────────

    #[test]
    fn validate_sync_url_http_ok() {
        assert!(validate_sync_url("http://sync.example.com").is_ok());
    }

    #[test]
    fn validate_sync_url_https_with_path_ok() {
        assert!(validate_sync_url("https://sync.example.com/v1/client/add-version/").is_ok());
    }

    #[test]
    fn validate_sync_url_https_with_port_ok() {
        assert!(validate_sync_url("https://localhost:8080").is_ok());
    }

    #[test]
    fn validate_sync_url_missing_schema() {
        let result = validate_sync_url("sync.example.com");
        assert!(matches!(result, Err(VmError::Sync { .. })));
    }

    #[test]
    fn validate_sync_url_wrong_schema() {
        let result = validate_sync_url("ftp://sync.example.com");
        assert!(matches!(result, Err(VmError::Sync { .. })));
    }

    #[test]
    fn validate_sync_url_empty_host() {
        // "http://" ohne Host
        let result = validate_sync_url("http://");
        assert!(matches!(result, Err(VmError::Sync { .. })));
    }
}
