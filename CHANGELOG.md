# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-07-17

### Added

- **Dependency editor** in the detail dialog: list existing `depends`
  relations with title lookup, remove them, add new ones from a picker of
  pending tasks (blocked/blocking flags update immediately).
- **Project hierarchy** in the sidebar: dotted projects (`Work.Sub`) render
  as a collapsible tree with implicit parents; counts use Taskwarrior
  prefix semantics (parent includes subprojects).
- **Legacy repair** maintenance action (Settings → Maintenance): converts
  token syntax left in task titles (`+tag project:x due:… priority:…`)
  into real properties; existing properties win, tokens fill gaps.
- **Synthetic interaction test** (`--test-input`): injects real
  `QMouseEvent`/`QKeyEvent` into the window (C++ shim with QTest-style fake
  timestamps) and verifies click selection, Ctrl/Shift multi-selection,
  checkbox toggle, double click → detail, right click → context menu, and
  real typing in quick capture. Runs in CI (offscreen).
- Live Secret Service roundtrip test (`cargo test -- --ignored secrets`).
- The demo dataset now contains a dotted subproject for hierarchy
  screenshots.

### Fixed

- Left-click handling on task rows was owned by the delegate button, so
  modifier clicks (Ctrl/Shift) never reached the selection logic — found by
  the new interaction test. Selection now uses a mouse overlay with
  explicit modifier handling; the done-checkbox stays natively clickable.
- The deprecated `KLocalizedContext` was replaced by
  `KLocalizedQmlContext` (KF ≥ 6.8).
- "MIT-Lizenz" in the About dialog is now translatable.

### Changed

- CI: `actions/checkout@v5`, new interaction-test step.

## [0.1.1] - 2026-07-17

### Fixed

- Placeholder arguments in localized strings rendered as
  `(I18N_ARGUMENT_MISSING)` (e.g. "Version %1" in the About dialog, date
  chips, bulk-delete confirmation, sync footer). ki18n substitutes `%1`
  itself — arguments are now passed to `i18n(...)` directly instead of
  `.arg()` chaining (10 call sites). Found by a fresh-context review; CI now
  greps the `--test-flow` log for `I18N_ARGUMENT_MISSING`.
- AppStream validation in CI no longer masked by `|| true`.

### Changed

- Settings note that standard dialog buttons (OK/Cancel) follow the system
  language, not the in-app language override.

## [0.1.0] - 2026-07-17

Initial release — KDE port of the [macOS app](https://github.com/hnsstrk/vergissmeinnicht),
feature-comparable for the daily-driver workflows.

### Added

- Kirigami UI with persistent sidebar: Inbox · Today · To Do · Overdue ·
  Due Soon · Scheduled · Waiting · All, plus per-project and per-tag rows with
  live counts and drop targets.
- Task list with working-set IDs, meta chips (priority, project, tags, due,
  scheduled, wait, recurrence, blocked/blocking, notes) and per-filter empty
  states.
- Full-text search with operators (`projekt:`/`project:`, `tag:`, `status:`
  with German and English aliases), AND terms, quoted phrases; store-wide scope
  while active. Saved searches pinned to the sidebar (rename/delete via
  context menu).
- Quick capture (Ctrl+N) with Taskwarrior token syntax (`+tag project:x
  due:tomorrow priority:H`), live token preview, and structured fields
  (notes, project, tags, due presets, priority, recurrence).
- Detail editor: title, project, tags, due, scheduled, wait, priority,
  recurrence (including custom `Nd/Nw/Nm/Ny`), annotations (add/remove),
  status with reactivate, dependency indicators.
- Multi-selection (Ctrl/Shift+click, Ctrl+A) with bulk done / delete /
  project / tag / priority / due / snooze via context menu; drag & drop onto
  sidebar projects, tags, and inbox.
- Recurring tasks: completing a task with `recur` + `due` atomically creates
  the follow-up instance (generator-light, same semantics as the macOS app).
- Snooze/wait with quick presets (tomorrow, +3 days, +1 week, clear).
- Sync against any taskchampion-sync-server; credentials in the system
  Secret Service (KWallet), auto-sync modes (manual/5m/15m/60m/immediate),
  sync status footer with local-changes indicator.
- Automatic `VACUUM INTO` backups before every sync (rotating, keep 10),
  manual backup/restore with pre-restore safety copy from the settings dialog.
- Opt-in overdue summary notification at launch (freedesktop notifications).
- German (source) and English localization via ki18n/gettext with in-app
  language override.
- Test hooks for headless verification: `--test-dialog=<name>`,
  `--test-grab=<file>`, `--test-flow` (scripted end-to-end smoke test).
- Packaging: desktop file, scalable icon, AppStream metainfo,
  `scripts/install-local.sh`, CI and release workflows (Arch container).

[Unreleased]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/releases/tag/v0.1.0
