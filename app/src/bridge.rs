//! cxx-qt-Bridge: `AppContainer` ist QAbstractListModel (sichtbare Task-Liste)
//! UND App-Fassade (Filter, Suche, Mutationen, Sync, Backups, Einstellungen) in
//! einem QObject — die Rust-Seite hält den `AppState` als Single Source of Truth.

// Die 8-/10-Parameter-Signaturen (addTaskDetailed, saveTaskDetail) sind durch
// die QML-Grenze bedingt — QML kann keine Structs übergeben.
#![allow(clippy::too_many_arguments)]

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::config::Settings;
use crate::filters::{SidebarFilter, SortOrder};
use crate::state::{status_key, AppState};
use crate::{backup, parsers, secrets};

use qobject::TaskRoles;

pub use qobject::{install_klocalized_context, set_ui_language};

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!(< QAbstractListModel >);
        type QAbstractListModel;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
    }

    unsafe extern "C++" {
        include!("klocalized.h");
        include!("cxx-qt-lib/qqmlengine.h");
        type QQmlEngine = cxx_qt_lib::QQmlEngine;

        /// Installiert den KLocalizedContext (ki18n) auf der QML-Engine —
        /// Voraussetzung für Kirigami Addons und unsere i18n()-Aufrufe.
        #[rust_name = "install_klocalized_context"]
        fn vmnInstallKLocalizedContext(engine: Pin<&mut QQmlEngine>);

        /// Erzwingt eine UI-Sprache (leer = Systemsprache); vor QML-Load aufrufen.
        #[rust_name = "set_ui_language"]
        fn vmnSetUiLanguage(language: &QString);

        include!("grabwindow.h");
        /// Rendert das Hauptfenster synchron in eine PNG-Datei (Testhaken).
        #[rust_name = "grab_first_window"]
        fn vmnGrabFirstWindow(path: &QString) -> bool;

        include!("inputsim.h");
        /// Synthetischer Mausklick ins Hauptfenster (Testhaken --test-input).
        #[rust_name = "send_click"]
        fn vmnSendClick(x: f64, y: f64, button: i32, modifiers: i32, double_click: bool);
        /// Synthetisches Tastatur-Event (Testhaken --test-input).
        #[rust_name = "send_key"]
        fn vmnSendKey(key: i32, modifiers: i32, text: &QString);
    }

    /// Modell-Rollen der Task-Liste.
    #[qenum(AppContainer)]
    enum TaskRoles {
        Uuid,
        WsId,
        Title,
        Project,
        TagsJson,
        Due,
        Scheduled,
        Wait,
        Priority,
        Recur,
        StatusKey,
        IsBlocked,
        IsBlocking,
        AnnotationCount,
        Entry,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, counts_json, cxx_name = "countsJson")]
        #[qproperty(QString, projects_json, cxx_name = "projectsJson")]
        #[qproperty(QString, tags_json, cxx_name = "tagsJson")]
        #[qproperty(QString, saved_searches_json, cxx_name = "savedSearchesJson")]
        #[qproperty(QString, filter_key, cxx_name = "filterKey")]
        #[qproperty(QString, search_query, cxx_name = "searchQuery")]
        #[qproperty(QString, sort_key, cxx_name = "sortKey")]
        #[qproperty(bool, sort_ascending, cxx_name = "sortAscending")]
        #[qproperty(bool, hide_completed, cxx_name = "hideCompleted")]
        #[qproperty(i32, due_soon_days, cxx_name = "dueSoonDays")]
        #[qproperty(QString, default_filter, cxx_name = "defaultFilter")]
        #[qproperty(QString, error_message, cxx_name = "errorMessage")]
        #[qproperty(QString, init_error, cxx_name = "initError")]
        #[qproperty(bool, is_syncing, cxx_name = "isSyncing")]
        #[qproperty(bool, sync_configured, cxx_name = "syncConfigured")]
        #[qproperty(bool, has_local_changes, cxx_name = "hasLocalChanges")]
        #[qproperty(i64, last_sync_at, cxx_name = "lastSyncAt")]
        #[qproperty(QString, sync_server_url, cxx_name = "syncServerUrl")]
        #[qproperty(QString, auto_sync_mode, cxx_name = "autoSyncMode")]
        #[qproperty(bool, notify_overdue, cxx_name = "notifyOverdue")]
        #[qproperty(i32, sidebar_width, cxx_name = "sidebarWidth")]
        #[qproperty(QString, collapsed_sections_json, cxx_name = "collapsedSectionsJson")]
        type AppContainer = super::AppContainerRust;

        // ── Modell-Overrides ────────────────────────────────────────────────

        #[cxx_override]
        #[rust_name = "row_count"]
        fn rowCount(&self, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(&self, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[rust_name = "role_names"]
        fn roleNames(&self) -> QHash_i32_QByteArray;

        #[inherit]
        #[rust_name = "begin_reset_model"]
        fn beginResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "end_reset_model"]
        fn endResetModel(self: Pin<&mut Self>);
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        // ── Ansicht / Filter / Suche / Sortierung ───────────────────────────
        #[qinvokable]
        fn refresh(self: Pin<&mut AppContainer>);
        #[qinvokable]
        fn apply_filter(self: Pin<&mut AppContainer>, key: &QString);
        #[qinvokable]
        fn apply_search(self: Pin<&mut AppContainer>, query: &QString);
        #[qinvokable]
        fn set_sort(self: Pin<&mut AppContainer>, key: &QString, ascending: bool);
        #[qinvokable]
        fn set_hide_completed_setting(self: Pin<&mut AppContainer>, hide: bool);
        #[qinvokable]
        fn set_due_soon_days_setting(self: Pin<&mut AppContainer>, days: i32);
        #[qinvokable]
        fn set_default_filter_setting(self: Pin<&mut AppContainer>, key: &QString);
        #[qinvokable]
        fn set_language_setting(self: Pin<&mut AppContainer>, language: &QString);
        #[qinvokable]
        fn language_setting(self: &AppContainer) -> QString;
        #[qinvokable]
        fn set_sidebar_width_setting(self: Pin<&mut AppContainer>, width: i32);
        #[qinvokable]
        fn set_collapsed_sections_setting(self: Pin<&mut AppContainer>, json: &QString);

        // ── Task-Detail ─────────────────────────────────────────────────────
        #[qinvokable]
        fn task_json(self: &AppContainer, uuid: &QString) -> QString;
        /// UUIDs der sichtbaren Zeilen im Index-Bereich [from, to] — für
        /// Shift+Klick-Bereichsauswahl in QML.
        #[qinvokable]
        fn visible_uuids(self: &AppContainer, from: i32, to: i32) -> QStringList;

        // ── Anlegen ─────────────────────────────────────────────────────────
        #[qinvokable]
        fn quick_capture_preview_json(self: &AppContainer, input: &QString) -> QString;
        #[qinvokable]
        fn quick_capture_commit(self: Pin<&mut AppContainer>, input: &QString) -> bool;
        #[qinvokable]
        fn add_task_detailed(
            self: Pin<&mut AppContainer>,
            title: &QString,
            project: &QString,
            tags_text: &QString,
            due: i64,
            priority: &QString,
            recur: &QString,
            notes: &QString,
        ) -> bool;

        // ── Einzel-Mutationen ───────────────────────────────────────────────
        #[qinvokable]
        fn mark_done(self: Pin<&mut AppContainer>, uuid: &QString);
        #[qinvokable]
        fn reactivate_task(self: Pin<&mut AppContainer>, uuid: &QString);
        #[qinvokable]
        fn save_task_detail(
            self: Pin<&mut AppContainer>,
            uuid: &QString,
            description: &QString,
            project: &QString,
            tags_text: &QString,
            due: i64,
            scheduled: i64,
            wait: i64,
            priority: &QString,
            recur: &QString,
        ) -> bool;
        #[qinvokable]
        fn add_task_annotation(self: Pin<&mut AppContainer>, uuid: &QString, text: &QString);
        #[qinvokable]
        fn remove_task_annotation(self: Pin<&mut AppContainer>, uuid: &QString, entry: i64);
        #[qinvokable]
        fn snooze_task(self: Pin<&mut AppContainer>, uuid: &QString, until: i64);
        /// Auswahlquelle für den Abhängigkeits-Editor: alle offenen Aufgaben.
        #[qinvokable]
        fn pending_tasks_json(self: &AppContainer) -> QString;
        #[qinvokable]
        fn add_task_dependency(self: Pin<&mut AppContainer>, uuid: &QString, depends_on: &QString);
        #[qinvokable]
        fn remove_task_dependency(self: Pin<&mut AppContainer>, uuid: &QString, depends_on: &QString);

        // ── Bulk-Aktionen ───────────────────────────────────────────────────
        #[qinvokable]
        fn mark_done_bulk(self: Pin<&mut AppContainer>, uuids: &QStringList);
        #[qinvokable]
        fn delete_tasks(self: Pin<&mut AppContainer>, uuids: &QStringList);
        #[qinvokable]
        fn bulk_set_project(self: Pin<&mut AppContainer>, uuids: &QStringList, project: &QString);
        #[qinvokable]
        fn bulk_add_tag(self: Pin<&mut AppContainer>, uuids: &QStringList, tag: &QString);
        #[qinvokable]
        fn bulk_set_priority(self: Pin<&mut AppContainer>, uuids: &QStringList, priority: &QString);
        #[qinvokable]
        fn bulk_set_due(self: Pin<&mut AppContainer>, uuids: &QStringList, due: i64);
        #[qinvokable]
        fn bulk_snooze(self: Pin<&mut AppContainer>, uuids: &QStringList, until: i64);

        // ── Drag & Drop / Sidebar-Management ────────────────────────────────
        #[qinvokable]
        fn drop_on_project(self: Pin<&mut AppContainer>, uuids: &QStringList, project: &QString);
        #[qinvokable]
        fn drop_on_tag(self: Pin<&mut AppContainer>, uuids: &QStringList, tag: &QString);
        #[qinvokable]
        fn drop_on_inbox(self: Pin<&mut AppContainer>, uuids: &QStringList);
        #[qinvokable]
        fn rename_project(self: Pin<&mut AppContainer>, old_name: &QString, new_name: &QString);
        #[qinvokable]
        fn clear_project(self: Pin<&mut AppContainer>, name: &QString);
        #[qinvokable]
        fn rename_tag(self: Pin<&mut AppContainer>, old_name: &QString, new_name: &QString);
        #[qinvokable]
        fn clear_tag(self: Pin<&mut AppContainer>, name: &QString);

        // ── Gespeicherte Suchen ─────────────────────────────────────────────
        #[qinvokable]
        fn save_current_search(self: Pin<&mut AppContainer>, name: &QString) -> bool;
        #[qinvokable]
        fn rename_saved_search(self: Pin<&mut AppContainer>, id: &QString, new_name: &QString);
        #[qinvokable]
        fn delete_saved_search(self: Pin<&mut AppContainer>, id: &QString);

        // ── Sync / Secrets ──────────────────────────────────────────────────
        #[qinvokable]
        fn start_sync(self: Pin<&mut AppContainer>);
        #[qinvokable]
        fn set_sync_server_url_setting(self: Pin<&mut AppContainer>, url: &QString);
        #[qinvokable]
        fn set_sync_credentials(self: Pin<&mut AppContainer>, client_id: &QString, secret: &QString) -> bool;
        #[qinvokable]
        fn sync_client_id(self: &AppContainer) -> QString;
        #[qinvokable]
        fn sync_secret(self: &AppContainer) -> QString;
        #[qinvokable]
        fn set_auto_sync_mode_setting(self: Pin<&mut AppContainer>, mode: &QString);

        /// Legacy-Reparatur: Token-Syntax in Descriptions → echte Properties.
        /// Rückgabe: Anzahl reparierter Aufgaben, -1 bei Fehler.
        #[qinvokable]
        fn repair_legacy_tasks(self: Pin<&mut AppContainer>) -> i32;

        // ── Backups ─────────────────────────────────────────────────────────
        #[qinvokable]
        fn backup_now(self: Pin<&mut AppContainer>) -> QString;
        #[qinvokable]
        fn backups_json(self: &AppContainer) -> QString;
        #[qinvokable]
        fn restore_backup_file(self: Pin<&mut AppContainer>, filename: &QString) -> bool;
        #[qinvokable]
        fn backup_folder(self: &AppContainer) -> QString;

        // ── Benachrichtigungen / Parser-Hilfen ──────────────────────────────
        #[qinvokable]
        fn set_notify_overdue_setting(self: Pin<&mut AppContainer>, enabled: bool);
        #[qinvokable]
        fn maybe_notify_overdue(self: Pin<&mut AppContainer>);
        /// Testhaken: rendert das Fenster in eine PNG-Datei (siehe --test-grab).
        #[qinvokable]
        fn grab_window_to(self: &AppContainer, path: &QString) -> bool;
        /// Testhaken: synthetischer Klick in Fensterkoordinaten (--test-input).
        #[qinvokable]
        fn test_click(self: &AppContainer, x: f64, y: f64, button: i32, modifiers: i32, double_click: bool);
        /// Testhaken: synthetisches Tastatur-Event (--test-input).
        #[qinvokable]
        fn test_key(self: &AppContainer, key: i32, modifiers: i32, text: &QString);
        #[qinvokable]
        fn parse_due_token(self: &AppContainer, token: &QString) -> i64;
        #[qinvokable]
        fn is_valid_recur_token(self: &AppContainer, token: &QString) -> bool;
        #[qinvokable]
        fn clear_error(self: Pin<&mut AppContainer>);
    }

    impl cxx_qt::Threading for AppContainer {}
    impl cxx_qt::Initialize for AppContainer {}
}

pub struct AppContainerRust {
    state: AppState,
    counts_json: QString,
    projects_json: QString,
    tags_json: QString,
    saved_searches_json: QString,
    filter_key: QString,
    search_query: QString,
    sort_key: QString,
    sort_ascending: bool,
    hide_completed: bool,
    due_soon_days: i32,
    default_filter: QString,
    error_message: QString,
    init_error: QString,
    is_syncing: bool,
    sync_configured: bool,
    has_local_changes: bool,
    last_sync_at: i64,
    sync_server_url: QString,
    auto_sync_mode: QString,
    notify_overdue: bool,
    sidebar_width: i32,
    collapsed_sections_json: QString,
}

impl Default for AppContainerRust {
    fn default() -> Self {
        let state = AppState::init();
        Self {
            counts_json: QString::default(),
            projects_json: QString::default(),
            tags_json: QString::default(),
            saved_searches_json: QString::default(),
            filter_key: QString::from(state.filter.to_key().as_str()),
            search_query: QString::default(),
            sort_key: QString::from(state.sort.to_key()),
            sort_ascending: state.sort_ascending,
            hide_completed: state.settings.hide_completed,
            due_soon_days: state.settings.due_soon_days as i32,
            default_filter: QString::from(state.settings.default_filter.as_str()),
            error_message: QString::default(),
            init_error: QString::from(state.init_error.clone().unwrap_or_default().as_str()),
            is_syncing: false,
            sync_configured: false,
            has_local_changes: false,
            last_sync_at: 0,
            sync_server_url: QString::from(state.settings.sync_server_url.as_str()),
            auto_sync_mode: QString::from(state.settings.auto_sync.as_str()),
            notify_overdue: state.settings.notify_overdue,
            sidebar_width: state.settings.sidebar_width as i32,
            collapsed_sections_json: QString::from(
                serde_json::to_string(&state.settings.collapsed_sections)
                    .unwrap_or_else(|_| "[]".into())
                    .as_str(),
            ),
            state,
        }
    }
}

impl cxx_qt::Initialize for qobject::AppContainer {
    fn initialize(mut self: Pin<&mut Self>) {
        let configured = compute_sync_configured(&self.rust().state.settings);
        self.as_mut().set_sync_configured(configured);
        self.as_mut().publish();
    }
}

/// Sync gilt als konfiguriert, wenn URL + Client-ID + Secret vorhanden sind.
/// Ergebnis wird gecacht (Secret-Service-Zugriff ist ein D-Bus-Roundtrip).
fn compute_sync_configured(settings: &Settings) -> bool {
    if settings.sync_server_url.trim().is_empty() {
        return false;
    }
    let has = |key: &str| matches!(secrets::get(key), Ok(Some(v)) if !v.is_empty());
    has(secrets::KEY_CLIENT_ID) && has(secrets::KEY_SECRET)
}

fn opt_string(q: &QString) -> Option<String> {
    let s = q.to_string();
    if s.trim().is_empty() {
        None
    } else {
        Some(s.trim().to_string())
    }
}

fn opt_secs(v: i64) -> Option<i64> {
    if v > 0 {
        Some(v)
    } else {
        None
    }
}

fn split_tags(text: &QString) -> Vec<String> {
    text.to_string()
        .split_whitespace()
        .map(|s| s.trim_start_matches('+').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn uuid_vec(list: &cxx_qt_lib::QStringList) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

impl qobject::AppContainer {
    // ─── Modell ─────────────────────────────────────────────────────────────

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.state.visible.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let role = TaskRoles { repr: role };
        let Some(task) = self.state.visible.get(index.row() as usize) else {
            return QVariant::default();
        };
        match role {
            TaskRoles::Uuid => QVariant::from(&QString::from(task.uuid.as_str())),
            TaskRoles::WsId => QVariant::from(&task.working_set_id.map(|i| i as i32).unwrap_or(-1)),
            TaskRoles::Title => QVariant::from(&QString::from(task.description.as_str())),
            TaskRoles::Project => {
                QVariant::from(&QString::from(task.project.clone().unwrap_or_default().as_str()))
            }
            TaskRoles::TagsJson => QVariant::from(&QString::from(
                serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".into()).as_str(),
            )),
            TaskRoles::Due => QVariant::from(&task.due.unwrap_or(0)),
            TaskRoles::Scheduled => QVariant::from(&task.scheduled.unwrap_or(0)),
            TaskRoles::Wait => QVariant::from(&task.wait.unwrap_or(0)),
            TaskRoles::Priority => {
                QVariant::from(&QString::from(task.priority.clone().unwrap_or_default().as_str()))
            }
            TaskRoles::Recur => {
                QVariant::from(&QString::from(task.recur.clone().unwrap_or_default().as_str()))
            }
            TaskRoles::StatusKey => QVariant::from(&QString::from(status_key(task.status))),
            TaskRoles::IsBlocked => QVariant::from(&task.is_blocked),
            TaskRoles::IsBlocking => QVariant::from(&task.is_blocking),
            TaskRoles::AnnotationCount => QVariant::from(&(task.annotations.len() as i32)),
            TaskRoles::Entry => QVariant::from(&task.entry.unwrap_or(0)),
            _ => QVariant::default(),
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut hash = QHash::<QHashPair_i32_QByteArray>::default();
        let pairs: [(TaskRoles, &str); 15] = [
            (TaskRoles::Uuid, "uuid"),
            (TaskRoles::WsId, "wsId"),
            (TaskRoles::Title, "title"),
            (TaskRoles::Project, "project"),
            (TaskRoles::TagsJson, "tagsJson"),
            (TaskRoles::Due, "due"),
            (TaskRoles::Scheduled, "scheduled"),
            (TaskRoles::Wait, "wait"),
            (TaskRoles::Priority, "priority"),
            (TaskRoles::Recur, "recur"),
            (TaskRoles::StatusKey, "statusKey"),
            (TaskRoles::IsBlocked, "isBlocked"),
            (TaskRoles::IsBlocking, "isBlocking"),
            (TaskRoles::AnnotationCount, "annotationCount"),
            (TaskRoles::Entry, "entry"),
        ];
        for (role, name) in pairs {
            hash.insert(role.repr, name.into());
        }
        hash
    }

    // ─── Interne Helfer ─────────────────────────────────────────────────────

    /// Führt eine Zustandsänderung unter Model-Reset aus, veröffentlicht danach
    /// alle abgeleiteten Properties und meldet Fehler in `errorMessage`.
    /// `is_mutation` steuert den „Sofort"-Auto-Sync.
    fn apply<F>(mut self: Pin<&mut Self>, f: F, is_mutation: bool) -> bool
    where
        F: FnOnce(&mut AppState) -> Result<(), String>,
    {
        self.as_mut().begin_reset_model();
        let result = f(&mut self.as_mut().rust_mut().state);
        self.as_mut().end_reset_model();
        let ok = match result {
            Ok(()) => {
                self.as_mut().set_error_message(QString::default());
                true
            }
            Err(e) => {
                self.as_mut().set_error_message(QString::from(e.as_str()));
                false
            }
        };
        self.as_mut().publish();
        if ok
            && is_mutation
            && self.auto_sync_mode.to_string() == "immediate"
            && *self.sync_configured()
            && !*self.is_syncing()
        {
            self.start_sync();
        }
        ok
    }

    /// Aktualisiert alle abgeleiteten Properties aus dem Zustand.
    fn publish(mut self: Pin<&mut Self>) {
        let counts = self.rust().state.counts_json();
        let projects = self.rust().state.projects_json();
        let tags = self.rust().state.tags_json();
        let saved = self.rust().state.saved_searches_json();
        let local_changes = if *self.sync_configured() {
            self.rust()
                .state
                .store
                .as_ref()
                .and_then(|s| s.num_local_operations().ok())
                .map(|n| n > 0)
                .unwrap_or(false)
        } else {
            false
        };
        self.as_mut().set_counts_json(QString::from(counts.as_str()));
        self.as_mut().set_projects_json(QString::from(projects.as_str()));
        self.as_mut().set_tags_json(QString::from(tags.as_str()));
        self.as_mut()
            .set_saved_searches_json(QString::from(saved.as_str()));
        self.as_mut().set_has_local_changes(local_changes);
    }

    // ─── Ansicht ────────────────────────────────────────────────────────────

    fn refresh(self: Pin<&mut Self>) {
        self.apply(|s| s.refresh(), false);
    }

    fn apply_filter(mut self: Pin<&mut Self>, key: &QString) {
        let filter = SidebarFilter::from_key(&key.to_string());
        // Saved Search aktivieren = Query setzen; jede andere Auswahl leert sie.
        let query = match &filter {
            SidebarFilter::SavedSearch(id) => {
                self.rust().state.saved_search_query(id).unwrap_or_default()
            }
            _ => String::new(),
        };
        self.as_mut().set_filter_key(QString::from(filter.to_key().as_str()));
        self.as_mut().set_search_query(QString::from(query.as_str()));
        self.apply(
            move |s| {
                s.filter = filter;
                s.search_query = query;
                s.rebuild_visible();
                Ok(())
            },
            false,
        );
    }

    fn apply_search(mut self: Pin<&mut Self>, query: &QString) {
        let query = query.to_string();
        // Manuell geleerte Suche verlässt eine aktive Saved Search.
        let reset_filter = query.trim().is_empty()
            && matches!(self.rust().state.filter, SidebarFilter::SavedSearch(_));
        if reset_filter {
            let default = SidebarFilter::from_key(&self.rust().state.settings.default_filter);
            self.as_mut()
                .set_filter_key(QString::from(default.to_key().as_str()));
            let q2 = query.clone();
            self.as_mut().set_search_query(QString::from(q2.as_str()));
            self.apply(
                move |s| {
                    s.filter = default;
                    s.search_query = q2;
                    s.rebuild_visible();
                    Ok(())
                },
                false,
            );
            return;
        }
        self.as_mut().set_search_query(QString::from(query.as_str()));
        self.apply(
            move |s| {
                s.search_query = query;
                s.rebuild_visible();
                Ok(())
            },
            false,
        );
    }

    fn set_sort(mut self: Pin<&mut Self>, key: &QString, ascending: bool) {
        let sort = SortOrder::from_key(&key.to_string());
        self.as_mut().set_sort_key(QString::from(sort.to_key()));
        self.as_mut().set_sort_ascending(ascending);
        self.apply(
            move |s| {
                s.sort = sort;
                s.sort_ascending = ascending;
                s.settings.sort_key = sort.to_key().to_string();
                s.settings.sort_ascending = ascending;
                let _ = s.settings.save();
                s.rebuild_visible();
                Ok(())
            },
            false,
        );
    }

    fn set_hide_completed_setting(mut self: Pin<&mut Self>, hide: bool) {
        self.as_mut().set_hide_completed(hide);
        self.apply(
            move |s| {
                s.settings.hide_completed = hide;
                let _ = s.settings.save();
                s.rebuild_visible();
                Ok(())
            },
            false,
        );
    }

    fn set_due_soon_days_setting(mut self: Pin<&mut Self>, days: i32) {
        let days = days.clamp(1, 60);
        self.as_mut().set_due_soon_days(days);
        self.apply(
            move |s| {
                s.settings.due_soon_days = days as i64;
                let _ = s.settings.save();
                s.rebuild_visible();
                Ok(())
            },
            false,
        );
    }

    fn set_default_filter_setting(mut self: Pin<&mut Self>, key: &QString) {
        let key = key.to_string();
        self.as_mut().set_default_filter(QString::from(key.as_str()));
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.default_filter = key;
        let _ = state.settings.save();
    }

    /// Sprach-Override persistieren (wirkt ab dem nächsten Start).
    fn set_language_setting(mut self: Pin<&mut Self>, language: &QString) {
        let language = language.to_string();
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.language = language;
        let _ = state.settings.save();
    }

    fn language_setting(&self) -> QString {
        QString::from(self.state.settings.language.as_str())
    }

    /// Vom Resize-Griff der Sidebar persistierte Breite (px); 0 = Theme-Standard.
    fn set_sidebar_width_setting(mut self: Pin<&mut Self>, width: i32) {
        let width = width.max(0);
        self.as_mut().set_sidebar_width(width);
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.sidebar_width = width as i64;
        let _ = state.settings.save();
    }

    /// Eingeklappte Sidebar-Sektionen persistieren (JSON-Array von Keys).
    fn set_collapsed_sections_setting(mut self: Pin<&mut Self>, json: &QString) {
        let parsed: Vec<String> = serde_json::from_str(&json.to_string()).unwrap_or_default();
        let normalized = serde_json::to_string(&parsed).unwrap_or_else(|_| "[]".into());
        self.as_mut()
            .set_collapsed_sections_json(QString::from(normalized.as_str()));
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.collapsed_sections = parsed;
        let _ = state.settings.save();
    }

    // ─── Detail / Anlegen ───────────────────────────────────────────────────

    fn task_json(&self, uuid: &QString) -> QString {
        QString::from(self.state.task_json(&uuid.to_string()).as_str())
    }

    fn visible_uuids(&self, from: i32, to: i32) -> cxx_qt_lib::QStringList {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        let lo = lo.max(0) as usize;
        let hi = hi.max(0) as usize;
        self.state
            .visible
            .iter()
            .skip(lo)
            .take(hi.saturating_sub(lo) + 1)
            .map(|t| QString::from(t.uuid.as_str()))
            .collect()
    }

    fn quick_capture_preview_json(&self, input: &QString) -> QString {
        let preview = parsers::parse_quick_capture(&input.to_string());
        let now = vergissmeinnicht_core::chrono::Utc::now().timestamp();
        let due_parsed = preview
            .due
            .as_deref()
            .map(|d| parsers::parse_due_date(d, now).is_some())
            .unwrap_or(true);
        let json = serde_json::json!({
            "description": preview.description,
            "tags": preview.tags,
            "project": preview.project,
            "due": preview.due,
            "priority": preview.priority,
            "dueParsed": due_parsed,
        });
        QString::from(json.to_string().as_str())
    }

    fn quick_capture_commit(self: Pin<&mut Self>, input: &QString) -> bool {
        let input = input.to_string();
        self.apply(move |s| s.quick_capture(&input), true)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_task_detailed(
        self: Pin<&mut Self>,
        title: &QString,
        project: &QString,
        tags_text: &QString,
        due: i64,
        priority: &QString,
        recur: &QString,
        notes: &QString,
    ) -> bool {
        let title = title.to_string().trim().to_string();
        let project = opt_string(project);
        let tags = split_tags(tags_text);
        let due = opt_secs(due);
        let priority = opt_string(priority);
        let recur = opt_string(recur);
        let notes = notes.to_string().trim().to_string();
        self.apply(
            move |s| {
                if title.is_empty() {
                    return Err("Titel darf nicht leer sein".into());
                }
                let Some(store) = &s.store else {
                    return Err("Store nicht initialisiert".into());
                };
                let uuid = store
                    .add_task_full(title, project, tags, due)
                    .map_err(|e| e.to_string())?;
                if let Some(p) = priority {
                    store.set_priority(uuid.clone(), Some(p)).map_err(|e| e.to_string())?;
                }
                if let Some(r) = recur {
                    store.set_recur(uuid.clone(), Some(r)).map_err(|e| e.to_string())?;
                }
                if !notes.is_empty() {
                    store.add_annotation(uuid, notes).map_err(|e| e.to_string())?;
                }
                s.refresh()
            },
            true,
        )
    }

    // ─── Einzel-Mutationen ──────────────────────────────────────────────────

    fn mark_done(self: Pin<&mut Self>, uuid: &QString) {
        let uuid = uuid.to_string();
        self.apply(move |s| s.mark_done_smart(&uuid), true);
    }

    fn reactivate_task(self: Pin<&mut Self>, uuid: &QString) {
        let uuid = uuid.to_string();
        self.apply(move |s| s.mutate(|store| store.reactivate(uuid.clone())), true);
    }

    /// Detail-Editor-Speichern: Description/Project/Tags/Due atomar, danach
    /// Scheduled/Wait/Priority/Recur einzeln (Port der macOS-Semantik inkl.
    /// `allSucceeded`-Akkumulation).
    #[allow(clippy::too_many_arguments)]
    fn save_task_detail(
        self: Pin<&mut Self>,
        uuid: &QString,
        description: &QString,
        project: &QString,
        tags_text: &QString,
        due: i64,
        scheduled: i64,
        wait: i64,
        priority: &QString,
        recur: &QString,
    ) -> bool {
        let uuid = uuid.to_string();
        let description = description.to_string().trim().to_string();
        let project = opt_string(project);
        let tags = split_tags(tags_text);
        let due = opt_secs(due);
        let scheduled = opt_secs(scheduled);
        let wait = opt_secs(wait);
        let priority = opt_string(priority);
        let recur = opt_string(recur);
        self.apply(
            move |s| {
                if description.is_empty() {
                    return Err("Titel darf nicht leer sein".into());
                }
                let Some(store) = &s.store else {
                    return Err("Store nicht initialisiert".into());
                };
                let mut errors: Vec<String> = Vec::new();
                if let Err(e) =
                    store.update_task_metadata(uuid.clone(), description, project, tags, due)
                {
                    errors.push(e.to_string());
                }
                for (result, label) in [
                    (store.set_scheduled(uuid.clone(), scheduled), "Geplant"),
                    (store.set_wait(uuid.clone(), wait), "Warten"),
                    (store.set_priority(uuid.clone(), priority), "Priorität"),
                    (store.set_recur(uuid.clone(), recur), "Wiederholung"),
                ] {
                    if let Err(e) = result {
                        errors.push(format!("{label}: {e}"));
                    }
                }
                let refresh_result = s.refresh();
                if !errors.is_empty() {
                    return Err(errors.join(" · "));
                }
                refresh_result
            },
            true,
        )
    }

    fn add_task_annotation(self: Pin<&mut Self>, uuid: &QString, text: &QString) {
        let uuid = uuid.to_string();
        let text = text.to_string();
        self.apply(
            move |s| s.mutate(|store| store.add_annotation(uuid.clone(), text.clone())),
            true,
        );
    }

    fn remove_task_annotation(self: Pin<&mut Self>, uuid: &QString, entry: i64) {
        let uuid = uuid.to_string();
        self.apply(
            move |s| s.mutate(|store| store.remove_annotation(uuid.clone(), entry)),
            true,
        );
    }

    fn snooze_task(self: Pin<&mut Self>, uuid: &QString, until: i64) {
        let uuid = uuid.to_string();
        let until = opt_secs(until);
        self.apply(
            move |s| s.mutate(|store| store.set_wait(uuid.clone(), until)),
            true,
        );
    }

    fn pending_tasks_json(&self) -> QString {
        QString::from(self.state.pending_tasks_json().as_str())
    }

    fn add_task_dependency(self: Pin<&mut Self>, uuid: &QString, depends_on: &QString) {
        let uuid = uuid.to_string();
        let dep = depends_on.to_string();
        self.apply(
            move |s| s.mutate(|store| store.add_dependency(uuid.clone(), dep.clone())),
            true,
        );
    }

    fn remove_task_dependency(self: Pin<&mut Self>, uuid: &QString, depends_on: &QString) {
        let uuid = uuid.to_string();
        let dep = depends_on.to_string();
        self.apply(
            move |s| s.mutate(|store| store.remove_dependency(uuid.clone(), dep.clone())),
            true,
        );
    }

    // ─── Bulk ───────────────────────────────────────────────────────────────

    fn mark_done_bulk(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList) {
        let uuids = uuid_vec(uuids);
        self.apply(
            move |s| {
                let mut errors = 0;
                for uuid in &uuids {
                    if s.mark_done_smart(uuid).is_err() {
                        errors += 1;
                    }
                }
                if errors > 0 {
                    Err(format!("{errors} Aufgabe(n) konnten nicht erledigt werden"))
                } else {
                    Ok(())
                }
            },
            true,
        );
    }

    fn delete_tasks(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList) {
        let uuids = uuid_vec(uuids);
        self.apply(
            move |s| {
                let Some(store) = s.store.clone() else {
                    return Err("Store nicht initialisiert".into());
                };
                let mut errors = 0;
                for uuid in &uuids {
                    if store.delete_task(uuid.clone()).is_err() {
                        errors += 1;
                    }
                }
                let result = s.refresh();
                if errors > 0 {
                    return Err(format!("{errors} Aufgabe(n) konnten nicht gelöscht werden"));
                }
                result
            },
            true,
        );
    }

    fn bulk_set_project(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, project: &QString) {
        let uuids = uuid_vec(uuids);
        let project = opt_string(project);
        self.bulk_apply(uuids, move |store, uuid| {
            store.set_project(uuid, project.clone())
        });
    }

    fn bulk_add_tag(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, tag: &QString) {
        let uuids = uuid_vec(uuids);
        let tag = tag.to_string();
        self.bulk_apply(uuids, move |store, uuid| store.add_tag(uuid, tag.clone()));
    }

    fn bulk_set_priority(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, priority: &QString) {
        let uuids = uuid_vec(uuids);
        let priority = opt_string(priority);
        self.bulk_apply(uuids, move |store, uuid| {
            store.set_priority(uuid, priority.clone())
        });
    }

    fn bulk_set_due(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, due: i64) {
        let uuids = uuid_vec(uuids);
        let due = opt_secs(due);
        self.bulk_apply(uuids, move |store, uuid| store.set_due(uuid, due));
    }

    fn bulk_snooze(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, until: i64) {
        let uuids = uuid_vec(uuids);
        let until = opt_secs(until);
        self.bulk_apply(uuids, move |store, uuid| store.set_wait(uuid, until));
    }

    // ─── Drag & Drop ────────────────────────────────────────────────────────

    fn drop_on_project(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, project: &QString) {
        self.bulk_set_project(uuids, project);
    }

    fn drop_on_tag(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList, tag: &QString) {
        self.bulk_add_tag(uuids, tag);
    }

    /// Drop auf Eingang: Projekt und ALLE Tags entfernen (macOS-Semantik).
    fn drop_on_inbox(self: Pin<&mut Self>, uuids: &cxx_qt_lib::QStringList) {
        let uuids = uuid_vec(uuids);
        self.apply(
            move |s| {
                let Some(store) = s.store.clone() else {
                    return Err("Store nicht initialisiert".into());
                };
                let mut errors = 0;
                for uuid in &uuids {
                    if store.set_project(uuid.clone(), None).is_err() {
                        errors += 1;
                        continue;
                    }
                    let tags: Vec<String> = s
                        .task_by_uuid(uuid)
                        .map(|t| t.tags.clone())
                        .unwrap_or_default();
                    for tag in tags {
                        if store.remove_tag(uuid.clone(), tag).is_err() {
                            errors += 1;
                        }
                    }
                }
                let result = s.refresh();
                if errors > 0 {
                    return Err(format!("{errors} Änderung(en) fehlgeschlagen"));
                }
                result
            },
            true,
        );
    }

    fn rename_project(self: Pin<&mut Self>, old_name: &QString, new_name: &QString) {
        let old = old_name.to_string();
        let new = new_name.to_string().trim().to_string();
        let update_filter = SidebarFilter::Project(new.clone());
        let was_active = {
            let key = self.filter_key().to_string();
            SidebarFilter::from_key(&key) == SidebarFilter::Project(old.clone())
        };
        let mut this = self;
        if was_active && !new.is_empty() {
            this.as_mut()
                .set_filter_key(QString::from(update_filter.to_key().as_str()));
        }
        this.apply(
            move |s| {
                if new.is_empty() {
                    return Err("Name darf nicht leer sein".into());
                }
                let result = s.rename_project(&old, &new);
                if was_active {
                    s.filter = SidebarFilter::Project(new.clone());
                    s.rebuild_visible();
                }
                result
            },
            true,
        );
    }

    fn clear_project(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        self.apply(move |s| s.clear_project(&name), true);
    }

    fn rename_tag(self: Pin<&mut Self>, old_name: &QString, new_name: &QString) {
        let old = old_name.to_string();
        let new = new_name.to_string().trim().to_string();
        let was_active = {
            let key = self.filter_key().to_string();
            SidebarFilter::from_key(&key) == SidebarFilter::Tag(old.clone())
        };
        let mut this = self;
        if was_active && !new.is_empty() {
            this.as_mut()
                .set_filter_key(QString::from(SidebarFilter::Tag(new.clone()).to_key().as_str()));
        }
        this.apply(
            move |s| {
                if new.is_empty() {
                    return Err("Name darf nicht leer sein".into());
                }
                let result = s.rename_tag(&old, &new);
                if was_active {
                    s.filter = SidebarFilter::Tag(new.clone());
                    s.rebuild_visible();
                }
                result
            },
            true,
        );
    }

    fn clear_tag(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        self.apply(move |s| s.clear_tag(&name), true);
    }

    // ─── Gespeicherte Suchen ────────────────────────────────────────────────

    fn save_current_search(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = name.to_string();
        let query = self.search_query().to_string();
        let result = self
            .as_mut()
            .rust_mut()
            .state
            .save_search(&name, &query);
        match result {
            Ok(id) => {
                self.as_mut().set_error_message(QString::default());
                // Aktivierte Saved Search direkt auswählen.
                let key = SidebarFilter::SavedSearch(id).to_key();
                self.as_mut().set_filter_key(QString::from(key.as_str()));
                self.as_mut().rust_mut().state.filter = SidebarFilter::from_key(&key);
                self.as_mut().publish();
                true
            }
            Err(e) => {
                self.as_mut().set_error_message(QString::from(e.as_str()));
                false
            }
        }
    }

    fn rename_saved_search(mut self: Pin<&mut Self>, id: &QString, new_name: &QString) {
        let id = id.to_string();
        let new_name = new_name.to_string();
        let result = self
            .as_mut()
            .rust_mut()
            .state
            .rename_saved_search(&id, &new_name);
        if let Err(e) = result {
            self.as_mut().set_error_message(QString::from(e.as_str()));
        }
        self.publish();
    }

    fn delete_saved_search(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let was_active = matches!(
            &self.rust().state.filter,
            SidebarFilter::SavedSearch(active) if *active == id
        );
        let result = self.as_mut().rust_mut().state.delete_saved_search(&id);
        if let Err(e) = result {
            self.as_mut().set_error_message(QString::from(e.as_str()));
        }
        if was_active {
            let default_key = self.rust().state.settings.default_filter.clone();
            self.as_mut().apply_filter(&QString::from(default_key.as_str()));
        } else {
            self.publish();
        }
    }

    // ─── Sync ───────────────────────────────────────────────────────────────

    /// Startet den Sync in einem Worker-Thread. Ohne Konfiguration wird nur
    /// aktualisiert (Port von `syncIfConfigured`).
    fn start_sync(mut self: Pin<&mut Self>) {
        if *self.is_syncing() {
            return;
        }
        if !*self.sync_configured() {
            self.refresh();
            return;
        }
        let Some(store) = self.rust().state.store.clone() else {
            return;
        };
        let url = self.rust().state.settings.sync_server_url.clone();
        let client_id = secrets::get(secrets::KEY_CLIENT_ID).ok().flatten().unwrap_or_default();
        let secret = secrets::get(secrets::KEY_SECRET).ok().flatten().unwrap_or_default();
        if client_id.is_empty() || secret.is_empty() {
            self.as_mut().set_sync_configured(false);
            self.refresh();
            return;
        }
        self.as_mut().set_is_syncing(true);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            // Auto-Backup vor jedem Sync (best effort).
            let backup_err = AppState::backup_before_sync();
            let sync_result = store
                .sync(url, client_id, secret)
                .map_err(|e| e.to_string());
            let _ = qt_thread.queue(move |mut qobject| {
                qobject.as_mut().set_is_syncing(false);
                match &sync_result {
                    Ok(()) => {
                        let now = vergissmeinnicht_core::chrono::Utc::now().timestamp();
                        qobject.as_mut().set_last_sync_at(now);
                        if let Some(e) = backup_err {
                            qobject
                                .as_mut()
                                .set_error_message(QString::from(format!("Backup: {e}").as_str()));
                        }
                    }
                    Err(e) => {
                        qobject
                            .as_mut()
                            .set_error_message(QString::from(format!("Sync: {e}").as_str()));
                    }
                }
                qobject.refresh();
            });
        });
    }

    fn set_sync_server_url_setting(mut self: Pin<&mut Self>, url: &QString) {
        let url = url.to_string().trim().to_string();
        self.as_mut().set_sync_server_url(QString::from(url.as_str()));
        {
            let state = &mut self.as_mut().rust_mut().state;
            state.settings.sync_server_url = url;
            let _ = state.settings.save();
        }
        let configured = compute_sync_configured(&self.rust().state.settings);
        self.as_mut().set_sync_configured(configured);
        self.publish();
    }

    fn set_sync_credentials(mut self: Pin<&mut Self>, client_id: &QString, secret: &QString) -> bool {
        let mut ok = true;
        if let Err(e) = secrets::set(secrets::KEY_CLIENT_ID, client_id.to_string().trim()) {
            self.as_mut()
                .set_error_message(QString::from(format!("Secret Service: {e}").as_str()));
            ok = false;
        }
        if let Err(e) = secrets::set(secrets::KEY_SECRET, secret.to_string().trim()) {
            self.as_mut()
                .set_error_message(QString::from(format!("Secret Service: {e}").as_str()));
            ok = false;
        }
        let configured = compute_sync_configured(&self.rust().state.settings);
        self.as_mut().set_sync_configured(configured);
        self.publish();
        ok
    }

    fn sync_client_id(&self) -> QString {
        QString::from(
            secrets::get(secrets::KEY_CLIENT_ID)
                .ok()
                .flatten()
                .unwrap_or_default()
                .as_str(),
        )
    }

    fn sync_secret(&self) -> QString {
        QString::from(
            secrets::get(secrets::KEY_SECRET)
                .ok()
                .flatten()
                .unwrap_or_default()
                .as_str(),
        )
    }

    fn set_auto_sync_mode_setting(mut self: Pin<&mut Self>, mode: &QString) {
        let mode = mode.to_string();
        self.as_mut().set_auto_sync_mode(QString::from(mode.as_str()));
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.auto_sync = mode;
        let _ = state.settings.save();
    }

    fn repair_legacy_tasks(mut self: Pin<&mut Self>) -> i32 {
        self.as_mut().begin_reset_model();
        let result = self.as_mut().rust_mut().state.repair_legacy_tasks();
        self.as_mut().end_reset_model();
        let count = match result {
            Ok(n) => {
                self.as_mut().set_error_message(QString::default());
                n as i32
            }
            Err(e) => {
                self.as_mut().set_error_message(QString::from(e.as_str()));
                -1
            }
        };
        self.publish();
        count
    }

    // ─── Backups ────────────────────────────────────────────────────────────

    fn backup_now(mut self: Pin<&mut Self>) -> QString {
        match backup::create_backup(
            &crate::config::replica_dir(),
            &crate::config::backup_dir(),
            "manual",
        ) {
            Ok(path) => {
                self.as_mut().set_error_message(QString::default());
                QString::from(path.to_string_lossy().as_ref())
            }
            Err(e) => {
                self.as_mut().set_error_message(QString::from(e.as_str()));
                QString::default()
            }
        }
    }

    fn backups_json(&self) -> QString {
        let entries = backup::list_backups(&crate::config::backup_dir());
        QString::from(serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into()).as_str())
    }

    /// Restore: Store schließen, Datei tauschen, Store neu öffnen. Während eines
    /// laufenden Syncs nicht erlaubt.
    fn restore_backup_file(mut self: Pin<&mut Self>, filename: &QString) -> bool {
        if *self.is_syncing() {
            self.as_mut().set_error_message(QString::from(
                "Wiederherstellung während eines Syncs nicht möglich",
            ));
            return false;
        }
        let filename = filename.to_string();
        self.apply(
            move |s| {
                // Verbindung schließen (letzte Arc-Referenz auf den Store).
                s.store = None;
                let restore = backup::restore_backup(
                    &crate::config::replica_dir(),
                    &crate::config::backup_dir(),
                    &filename,
                );
                // Unabhängig vom Ergebnis wieder öffnen.
                let reopen = vergissmeinnicht_core::TaskStore::new(
                    crate::config::replica_dir().to_string_lossy().into_owned(),
                );
                match reopen {
                    Ok(store) => {
                        s.store = Some(std::sync::Arc::new(store));
                        s.init_error = None;
                    }
                    Err(e) => {
                        s.init_error = Some(e.to_string());
                        return Err(format!("Replica neu öffnen fehlgeschlagen: {e}"));
                    }
                }
                restore?;
                s.refresh()
            },
            false,
        )
    }

    fn backup_folder(&self) -> QString {
        QString::from(crate::config::backup_dir().to_string_lossy().as_ref())
    }

    // ─── Benachrichtigungen / Hilfen ────────────────────────────────────────

    fn set_notify_overdue_setting(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_notify_overdue(enabled);
        let state = &mut self.as_mut().rust_mut().state;
        state.settings.notify_overdue = enabled;
        let _ = state.settings.save();
    }

    /// Zusammenfassungs-Notification überfälliger Aufgaben — nur wenn der Zähler
    /// seit dem letzten Lauf gestiegen ist (Anti-Spam, Port der macOS-Logik).
    fn maybe_notify_overdue(mut self: Pin<&mut Self>) {
        if !self.rust().state.settings.notify_overdue {
            return;
        }
        let count = self.rust().state.overdue_count();
        let last = self.rust().state.settings.last_overdue_count;
        {
            let state = &mut self.as_mut().rust_mut().state;
            state.settings.last_overdue_count = count as i64;
            let _ = state.settings.save();
        }
        if count > 0 && count as i64 > last {
            std::thread::spawn(move || {
                let body = if count == 1 {
                    "1 Aufgabe ist überfällig.".to_string()
                } else {
                    format!("{count} Aufgaben sind überfällig.")
                };
                let _ = notify_rust::Notification::new()
                    .summary("Vergissmeinnicht")
                    .body(&body)
                    .icon("de.hnsstrk.vergissmeinnicht")
                    .appname("Vergissmeinnicht")
                    .show();
            });
        }
    }

    fn grab_window_to(&self, path: &QString) -> bool {
        qobject::grab_first_window(path)
    }

    fn test_click(&self, x: f64, y: f64, button: i32, modifiers: i32, double_click: bool) {
        qobject::send_click(x, y, button, modifiers, double_click);
    }

    fn test_key(&self, key: i32, modifiers: i32, text: &QString) {
        qobject::send_key(key, modifiers, text);
    }

    fn parse_due_token(&self, token: &QString) -> i64 {
        let now = vergissmeinnicht_core::chrono::Utc::now().timestamp();
        parsers::parse_due_date(&token.to_string(), now).unwrap_or(0)
    }

    fn is_valid_recur_token(&self, token: &QString) -> bool {
        let t = token.to_string();
        t.trim().is_empty() || parsers::is_valid_recur(&t)
    }

    fn clear_error(mut self: Pin<&mut Self>) {
        self.as_mut().set_error_message(QString::default());
    }
}

impl qobject::AppContainer {
    /// Gemeinsamer Bulk-Pfad: Operation je UUID, ein Refresh, Teilfehler-Report.
    fn bulk_apply<F>(self: Pin<&mut Self>, uuids: Vec<String>, op: F)
    where
        F: Fn(&vergissmeinnicht_core::TaskStore, String) -> Result<(), vergissmeinnicht_core::VmError>,
    {
        self.apply(
            move |s| {
                let Some(store) = s.store.clone() else {
                    return Err("Store nicht initialisiert".into());
                };
                let mut errors = 0;
                for uuid in &uuids {
                    if op(&store, uuid.clone()).is_err() {
                        errors += 1;
                    }
                }
                let result = s.refresh();
                if errors > 0 {
                    return Err(format!("{errors} Änderung(en) fehlgeschlagen"));
                }
                result
            },
            true,
        );
    }
}
