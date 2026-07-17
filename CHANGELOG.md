# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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

[Unreleased]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/releases/tag/v0.1.0
