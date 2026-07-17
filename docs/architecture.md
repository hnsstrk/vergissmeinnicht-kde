# Architecture Notes

Design rationale for the storage layer and the bridge of Vergissmeinnicht
(KDE). This document explains *why* a few non-obvious choices look the way
they do; for the build toolchain read [`building.md`](building.md), and for
the layer overview read [`../README.md`](../README.md).

This port shares its storage design with the
[macOS app](https://github.com/hnsstrk/vergissmeinnicht) — the sections on
the working-set ID and the replica lifecycle apply to both code bases; the
public API of `vergissmeinnicht-core` is deliberately kept identical so fixes
can travel between the ports.

## Storage location

The on-disk replica lives at:

```
~/.local/share/vergissmeinnicht/replica/taskchampion.sqlite3
```

The app only asks for the *logical* XDG data location (`dirs::data_dir()`)
and appends `vergissmeinnicht/replica` — see `config.rs`. Configuration is a
JSON file under `~/.config/vergissmeinnicht/config.json`; backups live next
to the replica under `~/.local/share/vergissmeinnicht/backups/`.

**Why not `~/.task/`.** A local Taskwarrior CLI keeps its own TaskChampion
replica. Sharing one SQLite database between two concurrently running
programs invites lock contention and version skew, so the app deliberately
keeps its own replica — the same decision the sandboxed macOS app makes. The
two stores converge through the *same sync server*: the app syncs to a
user-configured `taskchampion-sync-server` over HTTPS, and the CLI syncs to
the same server. Sync is the only data channel.

**Secrets.** The sync client ID and encryption secret are stored in the
Secret Service (KWallet on Plasma) under the service name
`de.hnsstrk.vergissmeinnicht.sync` — never in the config file. The server URL
is not secret and lives in the config (macOS parity: Keychain vs.
UserDefaults).

## Working-set ID (`u32`)

`TaskInfo` in `core/src/lib.rs` carries:

```rust
pub working_set_id: Option<u32>,
```

This is the small numeric id Taskwarrior users know — the `1`, `2`, `3` … in
`task list`. It is derived from TaskChampion's *working set*, the index of
currently pending tasks; `list_tasks` fills it only inside the working-set
loop.

**Why `Option`.** The id exists only for tasks in the working set, i.e.
pending tasks. Completed, deleted, or recurring-master tasks have no
working-set index, so the field is `None` for them.

**Why a fixed-width integer.** `working_set().iter()` yields `usize` indices.
`usize` cannot cross the QML value boundary either (the bridge exposes the
role as `i32`, `-1` = none), so a fixed-width integer is used at the API
surface. `u32` trivially exceeds any realistic pending-task count.

**It is not a stable identifier.** The working-set id is recomputed on every
`list_tasks` call. It can change when tasks are completed, added, or the
working set is renumbered. The **stable** identity of a task is its UUID
(`TaskInfo.uuid`); the working-set id is a display convenience only. Any
persistence, cross-reference, or lookup must use the UUID.

## Replica lifecycle

The replica is **opened once** and kept alive for the lifetime of the process:

```rust
pub struct TaskStore {
    replica: Mutex<AppReplica>,
    rt: tokio::runtime::Runtime,
}
```

**Two distinct locks.**

- *In-process* — the Rust `Mutex<AppReplica>`. The `MutexGuard` is held across
  the entire `rt.block_on(...)` call; the Tokio runtime is intentionally
  **current-thread**. The mutex serialises all calls so no two run
  concurrently — including the sync worker thread, which holds the same
  `Arc<TaskStore>`. (On a poisoned mutex the guard is recovered via
  `into_inner()` rather than failing every later call until restart.)
- *Cross-process* — the SQLite file lock on the on-disk database, enforced by
  SQLite. A second app instance opening the same replica produces the
  "replica locked" failure.

**Commit.** Writes follow TaskChampion's operation model: each mutating call
builds an `Operations` batch, applies the changes to the in-memory task, then
commits the whole batch atomically via `commit_operations` — the single
durability point. `mark_done_with_followup` uses this to complete a recurring
task and create its successor in one atomic batch.

## Bridge design (cxx-qt)

One QObject — `AppContainer` — is both the QML list model
(`QAbstractListModel` base, roles for the visible tasks) and the app facade
(filters, search, mutations, sync, backups, settings). Rationale:

- The Rust side owns a single `AppState` (single source of truth: full task
  list, visible slice, filter/sort/search state, settings). Splitting model
  and facade into two QObjects would force shared ownership across the
  bridge for no benefit.
- Every mutation runs through one helper (`apply`) that wraps the change in
  `beginResetModel`/`endResetModel`, refreshes derived properties
  (counts/projects/tags/saved-searches JSON), and reports errors via the
  `errorMessage` property. Sidebar counts and the visible list use the same
  `SidebarFilter::matches` function, so they cannot drift.
- Sync runs on a worker thread (`cxx_qt::Threading`); results are queued back
  to the Qt thread. During a running sync the replica mutex serialises
  concurrent mutations.

QML receives aggregate data (counts, project/tag lists, saved searches,
task details) as JSON strings — one property change signal per rebuild, and
QML's `JSON.parse` is cheap at this scale (hundreds of tasks).

## Test hooks

Three command-line hooks exist for headless verification and screenshots
(they were essential during development, where the desktop session was
locked and no input injection was possible):

- `--test-dialog=<quickcapture|detail|settings|help|about>` opens the dialog
  after startup.
- `--test-grab=<file.png>` renders the window synchronously into a PNG via
  `QQuickWindow::grabWindow()` (works without compositor frame callbacks)
  and quits.
- `--test-flow` runs a scripted end-to-end smoke test through the real
  QML→bridge invokables (capture, search, edit, annotations, snooze,
  recurring follow-up, dependencies, legacy repair, bulk actions, saved
  searches, rename, delete) and prints `FLOW-OK`/`FLOW-FAIL` lines. Run it
  only against a disposable `XDG_DATA_HOME`.
- `--test-input` drives the UI with synthetic `QMouseEvent`/`QKeyEvent`
  injection (C++ shim): click selection, Ctrl/Shift multi-selection,
  checkbox toggle, double click → detail dialog, right click → context
  menu, and real typing into quick capture. This substitutes for a human
  mouse when no interactive session is available and runs in CI.
