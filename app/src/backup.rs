//! Backup-System — Port des macOS-`BackupService`.
//!
//! `VACUUM INTO` erzeugt eine konsistente Kopie der SQLite-Replica, ohne die
//! Live-Datenbank zu sperren oder einen Subprozess zu starten. Auto-Backup vor
//! jedem Sync, rotierend die letzten 10; Restore staged in ein Temp-Verzeichnis
//! und tauscht atomar (Rollback bei Fehler), mit `pre-restore`-Backup davor.

use std::path::{Path, PathBuf};

const DB_FILENAME: &str = "taskchampion.sqlite3";
const MAX_ROTATED: usize = 10;

fn db_path(replica_dir: &Path) -> PathBuf {
    replica_dir.join(DB_FILENAME)
}

fn timestamp_name(prefix: &str) -> String {
    let now = vergissmeinnicht_core::chrono::Local::now();
    format!("{prefix}-{}.sqlite3", now.format("%Y%m%d-%H%M%S"))
}

/// Erstellt ein Backup der Replica nach `backup_dir` und gibt den Pfad zurück.
/// `prefix` unterscheidet Auto- (`auto`), manuelle (`manual`) und
/// `pre-restore`-Backups.
pub fn create_backup(replica_dir: &Path, backup_dir: &Path, prefix: &str) -> Result<PathBuf, String> {
    let db = db_path(replica_dir);
    if !db.exists() {
        return Err(format!("Replica-Datenbank nicht gefunden: {}", db.display()));
    }
    std::fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    let target = backup_dir.join(timestamp_name(prefix));

    let conn = rusqlite::Connection::open_with_flags(
        &db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("Backup: DB öffnen fehlgeschlagen: {e}"))?;
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
        .map_err(|e| format!("Backup: VACUUM INTO fehlgeschlagen: {e}"))?;

    rotate(backup_dir, prefix);
    Ok(target)
}

/// Behalte pro Präfix nur die letzten `MAX_ROTATED` Backups (Dateiname sortiert
/// chronologisch dank Timestamp-Format).
fn rotate(backup_dir: &Path, prefix: &str) {
    let Ok(entries) = std::fs::read_dir(backup_dir) else { return };
    let mut names: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".sqlite3"))
                .unwrap_or(false)
        })
        .collect();
    names.sort();
    if names.len() > MAX_ROTATED {
        for old in &names[..names.len() - MAX_ROTATED] {
            let _ = std::fs::remove_file(old);
        }
    }
}

#[derive(serde::Serialize)]
pub struct BackupEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub modified: i64,
}

/// Alle Backups, neueste zuerst (für den Restore-Dialog).
pub fn list_backups(backup_dir: &Path) -> Vec<BackupEntry> {
    let Ok(entries) = std::fs::read_dir(backup_dir) else { return Vec::new() };
    let mut out: Vec<BackupEntry> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".sqlite3") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            Some(BackupEntry {
                filename: name,
                size_bytes: meta.len(),
                modified,
            })
        })
        .collect();
    out.sort_by(|a, b| b.filename.cmp(&a.filename));
    out
}

/// Stellt ein Backup wieder her. Ablauf (Datenverlust-sicher, Port der
/// macOS-Härtung): (1) `pre-restore`-Backup der Live-DB, (2) Backup-Datei in
/// eine Temp-Datei NEBEN der Live-DB kopieren, (3) atomarer Rename-Tausch mit
/// Rollback, (4) WAL-/SHM-Reste entfernen.
///
/// VORSICHT: Der Aufrufer muss den `TaskStore` vorher schließen/neu öffnen —
/// die Datei wird unter der laufenden Verbindung weggetauscht.
pub fn restore_backup(replica_dir: &Path, backup_dir: &Path, filename: &str) -> Result<(), String> {
    // Kein Pfad-Traversal: nur nackte Dateinamen aus list_backups akzeptieren.
    if filename.contains('/') || filename.contains("..") {
        return Err("Ungültiger Backup-Dateiname".into());
    }
    let source = backup_dir.join(filename);
    if !source.exists() {
        return Err(format!("Backup nicht gefunden: {}", source.display()));
    }
    let live = db_path(replica_dir);

    // (1) Sicherheitsnetz.
    create_backup(replica_dir, backup_dir, "pre-restore")?;

    // (2) Stage neben der Live-DB (gleiches Dateisystem → Rename ist atomar).
    let staged = replica_dir.join(format!("{DB_FILENAME}.restore-tmp"));
    std::fs::copy(&source, &staged).map_err(|e| format!("Restore: Kopie fehlgeschlagen: {e}"))?;

    // (3) Tausch mit Rollback.
    let old = replica_dir.join(format!("{DB_FILENAME}.pre-restore"));
    std::fs::rename(&live, &old).map_err(|e| format!("Restore: Live-DB sichern fehlgeschlagen: {e}"))?;
    if let Err(e) = std::fs::rename(&staged, &live) {
        // Rollback: alte DB zurück.
        let _ = std::fs::rename(&old, &live);
        return Err(format!("Restore: Einsetzen fehlgeschlagen (Rollback ausgeführt): {e}"));
    }
    let _ = std::fs::remove_file(&old);

    // (4) Stale WAL/SHM der alten DB würden die neue korrumpieren.
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(replica_dir.join(format!("{DB_FILENAME}{suffix}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_and_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let replica = dir.path().join("replica");
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&replica).unwrap();

        // Mini-SQLite-Datenbank als "Replica".
        {
            let conn = rusqlite::Connection::open(replica.join(DB_FILENAME)).unwrap();
            conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
            conn.execute("INSERT INTO t VALUES ('original')", []).unwrap();
        }

        let path = create_backup(&replica, &backups, "manual").unwrap();
        assert!(path.exists());

        // Live-DB verändern, dann Restore → alter Zustand zurück.
        {
            let conn = rusqlite::Connection::open(replica.join(DB_FILENAME)).unwrap();
            conn.execute("UPDATE t SET x = 'geändert'", []).unwrap();
        }
        let filename = path.file_name().unwrap().to_string_lossy().into_owned();
        restore_backup(&replica, &backups, &filename).unwrap();

        let conn = rusqlite::Connection::open(replica.join(DB_FILENAME)).unwrap();
        let x: String = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(x, "original");

        // pre-restore-Backup wurde angelegt.
        assert!(list_backups(&backups)
            .iter()
            .any(|b| b.filename.starts_with("pre-restore-")));
    }

    #[test]
    fn restore_rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let err = restore_backup(dir.path(), dir.path(), "../evil.sqlite3");
        assert!(err.is_err());
    }

    #[test]
    fn rotation_keeps_last_ten() {
        let dir = tempfile::tempdir().unwrap();
        let replica = dir.path().join("replica");
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&replica).unwrap();
        std::fs::create_dir_all(&backups).unwrap();
        {
            let conn = rusqlite::Connection::open(replica.join(DB_FILENAME)).unwrap();
            conn.execute("CREATE TABLE t (x)", []).unwrap();
        }
        // 12 künstliche alte Backups + 1 echtes → nur 10 bleiben.
        for i in 0..12 {
            std::fs::write(
                backups.join(format!("auto-2020010{}-00000{}.sqlite3", i % 10, i)),
                b"stub",
            )
            .unwrap();
        }
        create_backup(&replica, &backups, "auto").unwrap();
        let count = list_backups(&backups)
            .iter()
            .filter(|b| b.filename.starts_with("auto-"))
            .count();
        assert_eq!(count, MAX_ROTATED);
    }
}
