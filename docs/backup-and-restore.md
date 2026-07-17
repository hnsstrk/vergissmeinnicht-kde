# Backup & Restore

How Vergissmeinnicht protects the local replica, and how to recover it.
Port of the macOS backup system — same semantics, Linux paths.

## What gets backed up

The entire TaskChampion replica (`taskchampion.sqlite3`) as a consistent
SQLite snapshot created with `VACUUM INTO` — no subprocess, no partial
copies, safe while the app is running.

Locations:

| What | Path |
|------|------|
| Live replica | `~/.local/share/vergissmeinnicht/replica/` |
| Backups | `~/.local/share/vergissmeinnicht/backups/` |
| Config (not secret) | `~/.config/vergissmeinnicht/config.json` |
| Sync credentials | Secret Service (KWallet), service `de.hnsstrk.vergissmeinnicht.sync` |

## Automatic backups

Before **every sync** an `auto-<timestamp>.sqlite3` snapshot is written.
The last 10 backups per prefix are kept; older ones are rotated away.
A failed backup does not block the sync but is reported in the error banner.

## Manual backup

**Settings → Maintenance → Create backup now** writes a
`manual-<timestamp>.sqlite3` snapshot (also rotated, keep 10).
"Open backup folder" shows the files in your file manager.

## Restore

**Settings → Maintenance → Restore backup**:

1. A `pre-restore-<timestamp>.sqlite3` safety snapshot of the current live
   database is created first — a restore can always be undone.
2. The chosen backup is copied to a staging file **next to** the live
   database (same filesystem), then swapped in with an atomic rename.
   If the swap fails, the previous database is rolled back.
3. Stale SQLite WAL/SHM files are removed.
4. The store is closed before and reopened after the swap; the task list
   refreshes automatically.

Restores are refused while a sync is running.

## Manual recovery (app does not start)

If the replica is corrupted beyond what the in-app restore can fix:

```sh
cd ~/.local/share/vergissmeinnicht
mv replica/taskchampion.sqlite3 replica/taskchampion.sqlite3.broken
cp backups/<pick-a-backup>.sqlite3 replica/taskchampion.sqlite3
rm -f replica/taskchampion.sqlite3-wal replica/taskchampion.sqlite3-shm
```

Then start the app again. If you use a sync server, a fresh replica can also
be rebuilt from the server: move the `replica/` directory away, start the
app (it creates an empty replica), configure sync, and synchronize.

## What is *not* covered

- The sync server has its own storage and is not backed up by the app.
- Config and saved searches (`config.json`) are plain files — include them
  in your normal home-directory backup (e.g. restic).
