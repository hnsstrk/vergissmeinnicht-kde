//! Koexistenz-E2E: echte Taskwarrior-CLI und App-Replica am selben
//! taskchampion-sync-server. Verifiziert die harten Kompatibilitätszusagen:
//! UDAs/Fremdattribute überleben App-Edits, die App erzeugt keine
//! Recurrence-Duplikate neben der CLI-Engine.
//!
//! Läuft nur auf Anforderung (braucht `task`-CLI im PATH und einen laufenden
//! Sync-Server):
//!   cargo test -p vergissmeinnicht-core --test cli_coexistence -- --ignored
//! Server-URL via VMN_COEX_SERVER_URL (Default http://127.0.0.1:18085).

use std::process::Command;

use vergissmeinnicht_core::{chrono, TaskStore};

// Je Test eine eigene Client-ID — die Tests teilen sich den Server-Prozess,
// aber nicht den Datenbestand.
const CLIENT_ID_A: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";
const CLIENT_ID_B: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const SECRET: &str = "koexistenz-geheimnis";

fn server_url() -> String {
    std::env::var("VMN_COEX_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:18085".into())
}

struct Cli {
    taskdata: std::path::PathBuf,
    taskrc: std::path::PathBuf,
}

impl Cli {
    fn new(dir: &std::path::Path, client_id: &str) -> Self {
        let taskdata = dir.join("taskdata");
        std::fs::create_dir_all(&taskdata).unwrap();
        let taskrc = dir.join("taskrc");
        std::fs::write(
            &taskrc,
            format!(
                "sync.server.url={}\n\
                 sync.server.client_id={}\n\
                 sync.encryption_secret={}\n\
                 uda.estimate.type=string\n\
                 uda.estimate.label=Estimate\n\
                 confirmation=off\n\
                 recurrence.confirmation=no\n\
                 verbose=nothing\n",
                server_url(),
                client_id,
                SECRET
            ),
        )
        .unwrap();
        Self { taskdata, taskrc }
    }

    fn run(&self, args: &[&str]) -> String {
        let out = Command::new("task")
            .env("TASKDATA", &self.taskdata)
            .env("TASKRC", &self.taskrc)
            .args(args)
            .output()
            .expect("task-CLI nicht ausführbar — ist taskwarrior installiert?");
        assert!(
            out.status.success(),
            "task {args:?} schlug fehl:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

#[test]
#[ignore = "braucht task-CLI und laufenden Sync-Server (VMN_COEX_SERVER_URL)"]
fn cli_coexistence_preserves_udas_and_recurrence() {
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::new(dir.path(), CLIENT_ID_A);
    // Eindeutiger Lauf-Marker: der Server-Datenbestand überlebt Testläufe.
    let plain = format!("CLI-Task-{}", std::process::id());
    let rec = format!("CLI-Wiederkehrend-{}", std::process::id());

    // ── CLI legt Daten an und pusht ─────────────────────────────────────────
    cli.run(&["add", &plain, "project:coex", "estimate:3h"]);
    cli.run(&["add", &rec, "recur:daily", "due:today"]);
    // Report-Lauf zwingt die CLI, Recurrence-Instanzen zu generieren
    // (Achtung: rc.gc=off würde auch das Recurrence-Housekeeping unterdrücken).
    cli.run(&["list"]);
    cli.run(&["sync"]);

    // ── App zieht, editiert, pusht zurück ───────────────────────────────────
    let app_dir = dir.path().join("app-replica");
    let store = TaskStore::new(app_dir.to_string_lossy().into_owned()).unwrap();
    store
        .sync(server_url(), CLIENT_ID_A.into(), SECRET.into())
        .expect("App-Sync (pull)");

    let tasks = store.list_tasks(true).unwrap();
    let cli_task = tasks
        .iter()
        .find(|t| t.description == plain)
        .expect("CLI-Task angekommen");
    assert_eq!(cli_task.project.as_deref(), Some("coex"));

    let debug: Vec<String> = tasks
        .iter()
        .map(|t| format!("  {} status={:?} child={}", t.description, t.status, t.is_recurring_child))
        .collect();
    let child = tasks
        .iter()
        .find(|t| t.description == rec && t.is_recurring_child)
        .unwrap_or_else(|| {
            panic!(
                "Recurrence-Instanz (parent/imask) fehlt; App sieht:\n{}",
                debug.join("\n")
            )
        });
    let children_before = tasks
        .iter()
        .filter(|t| t.description == rec && t.is_recurring_child)
        .count();

    // App-Edits: Beschreibung + Priorität + Tag am UDA-Task; Instanz erledigen.
    // Der Tag prüft die Doppel-Repräsentation (kommagetrennte `tags`-Property
    // der CLI vs. `tag_<name>`-Flag von taskchampion).
    store
        .modify_description(cli_task.uuid.clone(), "App umbenannt".into())
        .unwrap();
    store
        .set_priority(cli_task.uuid.clone(), Some("H".into()))
        .unwrap();
    store
        .add_tag(cli_task.uuid.clone(), "appseitig".into())
        .unwrap();
    // Kern der Recurrence-Zusage: Instanz normal erledigen, KEIN eigener Followup.
    store.mark_done(child.uuid.clone()).unwrap();

    let after = store.list_tasks(true).unwrap();
    let pending_children_after = after
        .iter()
        .filter(|t| {
            t.description == rec
                && t.is_recurring_child
                && t.status == vergissmeinnicht_core::TaskStatus::Pending
        })
        .count();
    assert_eq!(
        pending_children_after,
        children_before - 1,
        "App darf keine eigene Folge-Instanz erzeugt haben"
    );

    store
        .sync(server_url(), CLIENT_ID_A.into(), SECRET.into())
        .expect("App-Sync (push)");

    // ── CLI zieht und verifiziert Unversehrtheit ────────────────────────────
    cli.run(&["sync"]);

    let uuid = &cli_task.uuid;
    let estimate = cli.run(&["_get", &format!("{uuid}.estimate")]);
    assert_eq!(estimate, "3h", "UDA muss App-Edits überleben");
    let desc = cli.run(&["_get", &format!("{uuid}.description")]);
    assert_eq!(desc, "App umbenannt");
    let prio = cli.run(&["_get", &format!("{uuid}.priority")]);
    assert_eq!(prio, "H");
    let tags = cli.run(&["_get", &format!("{uuid}.tags")]);
    assert!(
        tags.contains("appseitig"),
        "App-gesetzter Tag muss in der CLI sichtbar sein, tags={tags:?}"
    );

    // CLI generiert die nächste Instanz selbst; die App hat keine Duplikate
    // hinterlassen: alle offenen "CLI-Wiederkehrend" tragen einen parent.
    cli.run(&["list"]);
    let export = cli.run(&["rc.json.array=on", "export"]);
    let parsed: serde_json::Value = serde_json::from_str(&export).unwrap();
    let recurring_pending: Vec<&serde_json::Value> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            t["description"] == serde_json::Value::String(rec.clone())
                && t["status"] == "pending"
        })
        .collect();
    assert!(
        recurring_pending.iter().all(|t| t["parent"].is_string()),
        "jede offene Instanz muss CLI-verwaltet (parent gesetzt) sein — App-Duplikat gefunden:\n{export}"
    );
}

#[test]
#[ignore = "braucht task-CLI und laufenden Sync-Server (VMN_COEX_SERVER_URL)"]
fn app_owned_recur_task_is_harmless_for_cli() {
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::new(dir.path(), CLIENT_ID_B);
    // Leeren Stand herstellen (eigener Server-Datenbestand pro Client-ID wäre
    // sauberer; hier reicht ein eindeutiger Marker-Task).
    let app_dir = dir.path().join("app-replica");
    let store = TaskStore::new(app_dir.to_string_lossy().into_owned()).unwrap();

    // App-eigener recur-Task: pending + recur + due, OHNE parent/mask/rtype.
    let app_rec = format!("App-Wiederkehrend-{}", std::process::id());
    let uuid = store
        .add_task_full(
            app_rec.clone(),
            Some("coexapp".into()),
            vec![],
            Some(chrono::Utc::now().timestamp()),
        )
        .unwrap();
    store.set_recur(uuid.clone(), Some("weekly".into())).unwrap();
    store
        .sync(server_url(), CLIENT_ID_B.into(), SECRET.into())
        .expect("App-Sync (push)");

    cli.run(&["sync"]);
    // CLI-Sicht: Task erscheint als normaler Pending-Task; `task diagnostics`
    // läuft fehlerfrei; ein CLI-Report macht daraus KEIN Template und
    // generiert keine Instanzen.
    cli.run(&["list"]);
    cli.run(&["diagnostics"]);
    let export = cli.run(&["rc.json.array=on", "project:coexapp", "export"]);
    let parsed: serde_json::Value = serde_json::from_str(&export).unwrap();
    let matching: Vec<&serde_json::Value> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["description"] == serde_json::Value::String(app_rec.clone()))
        .collect();
    assert_eq!(matching.len(), 1, "kein Duplikat/keine Instanziierung:\n{export}");
    assert_eq!(matching[0]["status"], "pending");
    assert!(matching[0]["parent"].is_null(), "CLI darf keinen parent erfinden");

    // CLI erledigt den Task — muss ohne Fehler durchgehen.
    let cli_uuid = matching[0]["uuid"].as_str().unwrap();
    cli.run(&["rc.confirmation=off", cli_uuid, "done"]);
    cli.run(&["sync"]);
    store
        .sync(server_url(), CLIENT_ID_B.into(), SECRET.into())
        .expect("App-Sync (pull)");
    let after = store.list_tasks(true).unwrap();
    let t = after.iter().find(|t| t.uuid == uuid).expect("Task noch da");
    assert_eq!(t.status, vergissmeinnicht_core::TaskStatus::Completed);
}
