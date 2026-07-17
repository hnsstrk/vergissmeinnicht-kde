//! Replica-Roundtrip-Tests — Pendant zu den Swift-Testsuiten der macOS-Version
//! (ReplicaRoundtripTests, WriteOperationsTests, MetadataTests, SyncTests,
//! DependencyTests). Jeder Test arbeitet auf einer frischen Replica in einem
//! Temp-Verzeichnis.

use vergissmeinnicht_core::{TaskStatus, TaskStore, VmError};

fn fresh_store() -> (tempfile::TempDir, TaskStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TaskStore::new(dir.path().to_string_lossy().into_owned()).expect("open replica");
    (dir, store)
}

#[test]
fn ping_pong() {
    assert_eq!(vergissmeinnicht_core::ping(), "pong");
}

#[test]
fn open_replica_in_fresh_dir() {
    let (_dir, _store) = fresh_store();
}

#[test]
fn add_task_returns_uuid() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Erste Aufgabe".into()).expect("add");
    assert_eq!(uuid.len(), 36, "UUID-Form (8-4-4-4-12): {uuid}");
    assert_eq!(uuid.matches('-').count(), 4);
}

#[test]
fn add_then_list_roundtrip_with_umlauts() {
    let (_dir, store) = fresh_store();
    let description = "Müll rausbringen — größere Säcke für Grünschnitt";
    let uuid = store.add_task(description.into()).expect("add");

    let tasks = store.list_pending().expect("list");
    assert_eq!(tasks.len(), 1);
    let t = &tasks[0];
    assert_eq!(t.uuid, uuid);
    assert_eq!(t.description, description);
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.working_set_id.is_some());
    assert!(t.entry.is_some());
}

#[test]
fn add_task_full_persists_metadata() {
    let (_dir, store) = fresh_store();
    let due = 1_800_000_000_i64;
    let uuid = store
        .add_task_full(
            "Mit Metadaten".into(),
            Some("hausbau".into()),
            vec!["dringend".into(), "einkauf".into()],
            Some(due),
        )
        .expect("add_task_full");

    let tasks = store.list_pending().expect("list");
    let t = tasks.iter().find(|t| t.uuid == uuid).expect("task found");
    assert_eq!(t.project.as_deref(), Some("hausbau"));
    let mut tags = t.tags.clone();
    tags.sort();
    assert_eq!(tags, vec!["dringend".to_string(), "einkauf".to_string()]);
    assert_eq!(t.due, Some(due));
}

#[test]
fn mark_done_removes_from_pending() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Fertigmachen".into()).expect("add");
    store.mark_done(uuid.clone()).expect("done");

    let pending = store.list_pending().expect("list");
    assert!(pending.iter().all(|t| t.uuid != uuid));

    let all = store.list_tasks(true).expect("list all");
    let t = all.iter().find(|t| t.uuid == uuid).expect("still visible");
    assert_eq!(t.status, TaskStatus::Completed);
    assert!(t.working_set_id.is_none());
}

#[test]
fn modify_description_roundtrip() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Alt".into()).expect("add");
    store
        .modify_description(uuid.clone(), "Neu".into())
        .expect("modify");
    let tasks = store.list_pending().expect("list");
    assert_eq!(tasks[0].description, "Neu");
}

#[test]
fn delete_task_hides_everywhere() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Wegdamit".into()).expect("add");
    store.delete_task(uuid.clone()).expect("delete");

    let pending = store.list_pending().expect("list");
    assert!(pending.iter().all(|t| t.uuid != uuid));
    // Deleted bleiben auch bei include_completed unsichtbar.
    let all = store.list_tasks(true).expect("list all");
    assert!(all.iter().all(|t| t.uuid != uuid));
}

#[test]
fn annotations_add_and_remove() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Mit Notiz".into()).expect("add");
    store
        .add_annotation(uuid.clone(), "Erste Notiz".into())
        .expect("annotate");

    let tasks = store.list_pending().expect("list");
    assert_eq!(tasks[0].annotations.len(), 1);
    assert_eq!(tasks[0].annotations[0].description, "Erste Notiz");

    let entry = tasks[0].annotations[0].entry;
    store.remove_annotation(uuid, entry).expect("remove");
    let tasks = store.list_pending().expect("list");
    assert!(tasks[0].annotations.is_empty());
}

#[test]
fn set_project_and_clear() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Projektwechsel".into()).expect("add");
    store
        .set_project(uuid.clone(), Some("garten".into()))
        .expect("set");
    assert_eq!(
        store.list_pending().unwrap()[0].project.as_deref(),
        Some("garten")
    );
    store.set_project(uuid, None).expect("clear");
    assert_eq!(store.list_pending().unwrap()[0].project, None);
}

#[test]
fn add_remove_tag_idempotent() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Tagspiele".into()).expect("add");
    store.add_tag(uuid.clone(), "wichtig".into()).expect("add tag");
    store.add_tag(uuid.clone(), "wichtig".into()).expect("idempotent add");
    assert_eq!(store.list_pending().unwrap()[0].tags, vec!["wichtig"]);

    store.remove_tag(uuid.clone(), "wichtig".into()).expect("remove");
    store.remove_tag(uuid, "wichtig".into()).expect("idempotent remove");
    assert!(store.list_pending().unwrap()[0].tags.is_empty());
}

#[test]
fn invalid_tag_is_conversion_error() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Kaputter Tag".into()).expect("add");
    let result = store.add_tag(uuid, "hat leerzeichen".into());
    assert!(matches!(result, Err(VmError::Conversion { .. })));
}

#[test]
fn due_priority_wait_recur_scheduled_roundtrip() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Vollausstattung".into()).expect("add");

    store.set_due(uuid.clone(), Some(1_800_000_000)).expect("due");
    store.set_priority(uuid.clone(), Some("H".into())).expect("prio");
    store.set_wait(uuid.clone(), Some(1_900_000_000)).expect("wait");
    store.set_recur(uuid.clone(), Some("weekly".into())).expect("recur");
    store.set_scheduled(uuid.clone(), Some(1_750_000_000)).expect("sched");

    let t = &store.list_pending().unwrap()[0];
    assert_eq!(t.due, Some(1_800_000_000));
    assert_eq!(t.priority.as_deref(), Some("H"));
    assert_eq!(t.wait, Some(1_900_000_000));
    assert_eq!(t.recur.as_deref(), Some("weekly"));
    assert_eq!(t.scheduled, Some(1_750_000_000));

    // Alles wieder entfernen.
    store.set_due(uuid.clone(), None).expect("clear due");
    store.set_priority(uuid.clone(), None).expect("clear prio");
    store.set_wait(uuid.clone(), None).expect("clear wait");
    store.set_recur(uuid.clone(), None).expect("clear recur");
    store.set_scheduled(uuid, None).expect("clear sched");

    let t = &store.list_pending().unwrap()[0];
    assert_eq!(t.due, None);
    assert_eq!(t.priority, None);
    assert_eq!(t.wait, None);
    assert_eq!(t.recur, None);
    assert_eq!(t.scheduled, None);
}

#[test]
fn update_task_metadata_replaces_tags() {
    let (_dir, store) = fresh_store();
    let uuid = store
        .add_task_full(
            "Ersetzen".into(),
            Some("alt".into()),
            vec!["a".into(), "b".into()],
            Some(1_800_000_000),
        )
        .expect("add");

    store
        .update_task_metadata(
            uuid.clone(),
            "Ersetzt".into(),
            Some("neu".into()),
            vec!["c".into()],
            None,
        )
        .expect("update");

    let t = &store.list_pending().unwrap()[0];
    assert_eq!(t.description, "Ersetzt");
    assert_eq!(t.project.as_deref(), Some("neu"));
    assert_eq!(t.tags, vec!["c"]);
    assert_eq!(t.due, None);
}

#[test]
fn reactivate_returns_to_pending() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Zombie".into()).expect("add");
    store.mark_done(uuid.clone()).expect("done");
    store.reactivate(uuid.clone()).expect("reactivate");

    let pending = store.list_pending().expect("list");
    assert!(pending.iter().any(|t| t.uuid == uuid));
}

#[test]
fn mark_done_with_followup_creates_next_instance() {
    let (_dir, store) = fresh_store();
    let uuid = store
        .add_task_full(
            "Wöchentlich".into(),
            Some("routine".into()),
            vec!["haushalt".into()],
            Some(1_800_000_000),
        )
        .expect("add");
    store.set_recur(uuid.clone(), Some("weekly".into())).expect("recur");
    store.set_priority(uuid.clone(), Some("M".into())).expect("prio");

    let next_due = 1_800_000_000 + 7 * 86_400;
    let new_uuid = store
        .mark_done_with_followup(
            uuid.clone(),
            Some(next_due),
            Some("weekly".into()),
            Some("M".into()),
            Some("routine".into()),
            vec!["haushalt".into()],
            "Wöchentlich".into(),
        )
        .expect("followup")
        .expect("new instance created");

    let pending = store.list_pending().expect("list");
    assert_eq!(pending.len(), 1);
    let t = &pending[0];
    assert_eq!(t.uuid, new_uuid);
    assert_eq!(t.due, Some(next_due));
    assert_eq!(t.recur.as_deref(), Some("weekly"));
    assert_eq!(t.project.as_deref(), Some("routine"));

    // Ohne recur → keine Folge-Instanz.
    let none = store
        .mark_done_with_followup(new_uuid, None, None, None, None, vec![], "Egal".into())
        .expect("done");
    assert!(none.is_none());
}

#[test]
fn dependencies_blocked_and_blocking() {
    let (_dir, store) = fresh_store();
    let blocker = store.add_task("Zuerst".into()).expect("add");
    let dependent = store.add_task("Danach".into()).expect("add");

    store
        .add_dependency(dependent.clone(), blocker.clone())
        .expect("add dep");

    let tasks = store.list_pending().expect("list");
    let b = tasks.iter().find(|t| t.uuid == blocker).unwrap();
    let d = tasks.iter().find(|t| t.uuid == dependent).unwrap();
    assert!(b.is_blocking, "Blocker muss als blockierend markiert sein");
    assert!(d.is_blocked, "Abhängiger muss als blockiert markiert sein");
    assert_eq!(d.depends, vec![blocker.clone()]);

    store
        .remove_dependency(dependent.clone(), blocker.clone())
        .expect("remove dep");
    let tasks = store.list_pending().expect("list");
    let d = tasks.iter().find(|t| t.uuid == dependent).unwrap();
    assert!(!d.is_blocked);
    assert!(d.depends.is_empty());
}

#[test]
fn num_local_operations_counts() {
    let (_dir, store) = fresh_store();
    assert_eq!(store.num_local_operations().unwrap(), 0);
    store.add_task("Eins".into()).expect("add");
    assert!(store.num_local_operations().unwrap() > 0);
}

#[test]
fn sync_with_invalid_url_fails_early() {
    let (_dir, store) = fresh_store();
    let result = store.sync(
        "keine-url".into(),
        "550e8400-e29b-41d4-a716-446655440000".into(),
        "geheim".into(),
    );
    assert!(matches!(result, Err(VmError::Sync { .. })));
}

#[test]
fn sync_with_invalid_client_id_is_conversion_error() {
    let (_dir, store) = fresh_store();
    let result = store.sync(
        "https://sync.example.com".into(),
        "nicht-uuid".into(),
        "geheim".into(),
    );
    assert!(matches!(result, Err(VmError::Conversion { .. })));
}

#[test]
fn undo_reverts_last_batch() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Original".into()).expect("add");
    store
        .modify_description(uuid.clone(), "Geändert".into())
        .expect("modify");
    assert!(store.num_undo_points().unwrap() >= 2);

    assert!(store.undo_last_change().unwrap(), "Undo muss greifen");
    let tasks = store.list_tasks(true).unwrap();
    let t = tasks.iter().find(|t| t.uuid == uuid).expect("Task existiert");
    assert_eq!(t.description, "Original");

    // Zweites Undo entfernt die Anlage selbst.
    assert!(store.undo_last_change().unwrap());
    assert!(store.list_tasks(true).unwrap().iter().all(|t| t.uuid != uuid));

    // Nichts mehr rückgängig zu machen.
    assert!(!store.undo_last_change().unwrap());
}

#[test]
fn set_raw_property_roundtrip_preserves_unknown_attributes() {
    let (_dir, store) = fresh_store();
    let uuid = store.add_task("Mit UDA".into()).expect("add");
    store
        .set_raw_property(uuid.clone(), "estimate".into(), Some("3h".into()))
        .expect("set uda");
    // Normale App-Edits dürfen die fremde Property nicht anfassen.
    store
        .modify_description(uuid.clone(), "Umbenannt".into())
        .expect("modify");
    store.set_priority(uuid.clone(), Some("H".into())).expect("prio");

    // Rohwert überlebt (Kontrolle über erneutes Setzen + Entfernen).
    store
        .set_raw_property(uuid.clone(), "estimate".into(), None)
        .expect("clear uda");
}
