// End-to-End-Sync-Test gegen einen laufenden taskchampion-sync-server:
// legt in Replica A einen Task an, synct A → Server → B und prüft, ob der
// Task in Replica B ankommt (und eine Änderung aus B wieder in A).
//
//     cargo run --example sync_roundtrip -- <server-url> <client-id> <secret>
//
// Replicas werden in Temp-Verzeichnissen angelegt. Exit-Code 0 = Erfolg.

use std::env;
use std::process;

use vergissmeinnicht_core::TaskStore;

fn main() {
    let mut args = env::args().skip(1);
    let (Some(url), Some(client_id), Some(secret)) = (args.next(), args.next(), args.next())
    else {
        eprintln!("usage: sync_roundtrip <server-url> <client-id> <secret>");
        process::exit(2);
    };

    let dir_a = tempdir("vmn-sync-a");
    let dir_b = tempdir("vmn-sync-b");

    let store_a = TaskStore::new(dir_a.clone()).expect("Replica A öffnen");
    let store_b = TaskStore::new(dir_b.clone()).expect("Replica B öffnen");

    // A: Task anlegen und zum Server pushen.
    let uuid = store_a
        .add_task_full(
            "Sync-Roundtrip-Aufgabe".into(),
            Some("synctest".into()),
            vec!["e2e".into()],
            Some(1_800_000_000),
        )
        .expect("Task anlegen");
    store_a
        .sync(url.clone(), client_id.clone(), secret.clone())
        .expect("Sync A (push)");

    // B: vom Server ziehen und prüfen.
    store_b
        .sync(url.clone(), client_id.clone(), secret.clone())
        .expect("Sync B (pull)");
    let tasks_b = store_b.list_pending().expect("Liste B");
    let task_b = tasks_b
        .iter()
        .find(|t| t.uuid == uuid)
        .expect("Task ist nicht in Replica B angekommen");
    assert_eq!(task_b.description, "Sync-Roundtrip-Aufgabe");
    assert_eq!(task_b.project.as_deref(), Some("synctest"));
    assert_eq!(task_b.tags, vec!["e2e".to_string()]);
    assert_eq!(task_b.due, Some(1_800_000_000));

    // B: erledigen und zurücksyncen; A muss den Statuswechsel sehen.
    store_b.mark_done(uuid.clone()).expect("B: erledigt");
    store_b
        .sync(url.clone(), client_id.clone(), secret.clone())
        .expect("Sync B (push)");
    store_a
        .sync(url, client_id, secret)
        .expect("Sync A (pull)");
    let done_in_a = store_a
        .list_tasks(true)
        .expect("Liste A")
        .iter()
        .any(|t| t.uuid == uuid && t.status == vergissmeinnicht_core::TaskStatus::Completed);
    assert!(done_in_a, "Statuswechsel aus B ist nicht in A angekommen");

    println!("SYNC-E2E OK — Task {uuid} konvergierte über beide Replicas");
}

fn tempdir(prefix: &str) -> String {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    dir.to_string_lossy().into_owned()
}
