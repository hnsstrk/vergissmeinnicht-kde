# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Selectable AI context level (AI-B1b, #31): a new "Context scope for
  interpretation" combo on the AI settings page controls how much task
  data the AI sees when interpreting free-form input — project and tag
  names only (default, previous behavior), plus the titles of all open
  tasks, or all non-deleted tasks compactly (title, project, tags, due
  date, completed marker). Deleted tasks are never sent at any level, and
  each option's description names exactly what leaves the machine; for
  non-localhost endpoints the privacy note calls out that the larger
  levels transmit task content. The system prompt now puts volatile
  content (current date and time) after the stable schema, taxonomy and
  task list so backends can reuse their prompt prefix cache, and the task
  list is capped by size — completed tasks are dropped oldest first, open
  tasks are always kept, and the shortening is stated in the prompt. The
  new `aiCapturePromptPreview` invokable exposes the exact system prompt
  for verification and transparency.

- AI settings page (AI-A4, #12): the AI assistant is configured in the UI
  instead of hand-editing `config.json` — provider preset (Ollama /
  OpenRouter / custom, prefilling the base URL), base URL field, **model
  selection from the endpoint's model list** ("Load models" fetches
  `/v1/models` on a worker thread; the configured model stays selected
  even when it is missing from the list, and manual entry keeps working
  when the backend is offline), API key stored in the Secret Service, and
  the speech-to-text backend with per-backend fields. "Save and test"
  persists everything and verifies the connection via the model list,
  reporting into an in-dialog status line; a privacy note appears when the
  base URL is not localhost. Changing provider, base URL, model or API key
  invalidates the cached LLM client (the documented AI-A3 open point,
  covered by a Rust test), and `aiConfigured` updates live so quick
  capture's AI controls appear without a restart.
- Quick capture can interpret free-form input with AI (AI-B1, #11): with a
  configured backend, an "Interpret with AI" action (Ctrl+J, listed in the
  help dialog) sends the title text plus the current date and the
  project/tag taxonomy to the model and fills the structured form fields
  from the validated response — due dates and recurrence are checked with
  the existing parsers, priority against H/M/L, and invalid values leave
  their fields empty. Absolute ISO dates land in the date picker, other
  valid expressions in the custom fields; new project names are allowed.
  The AI only proposes: tasks are still created through the regular Add
  button. Errors appear in a dialog-local message bound to the dedicated
  `aiError` channel, a busy indicator shows a running request, and the
  headless `--test-flow` covers the fill function and the end-to-end path
  against the canned-response mock.
- AI groundwork, not yet user-visible (AI-A3, #10): the bridge exposes
  `aiConfigured`, `aiBusy`, `aiError`, `aiResponseJson` and
  `dictationAvailable`, runs LLM requests on a worker thread
  (`start_sync` pattern) and drops stale or cancelled responses via a
  generation counter — the newest request wins, results publish as plain
  property sets without a model reset, and AI errors never touch the
  global error banner. `cancelAiRequest` aborts; the AI API key can be
  stored in the Secret Service via `setAiApiKey`. The headless
  `--test-flow` covers the scaffolding end-to-end against the canned-
  response mock (`VMN_AI_MOCK`).
- Quick capture: the due and recurrence combos gained a "Custom …" entry
  (pattern from the detail editor). Due accepts any Taskwarrior date
  expression (`+3d`, `eow`, ISO date) in a validated text field;
  recurrence accepts free intervals such as `3d`, `2w` or `quarterly`
  (AI-B0, #6).
- Search: free-text terms are now typo-tolerant. German spelling variants
  are folded (`ue↔ü`, `ae↔ä`, `oe↔ö`, `ss↔ß`), so `pruefen` finds "prüfen"
  and `strasse` finds "Straße"; a bounded edit distance (1 for words of
  four to seven letters, 2 beyond that, none for shorter ones) catches
  slips such as `prüfem`. Exact substring matching runs first and is
  unchanged, and `project:`/`tag:`/`status:` stay exact (UI-3, #29).

### Fixed

- The AI request timeout was 120 s, which a local reasoning model exceeds:
  measured against Ollama with `qwen3.6:27b`, a single interpretation took
  90-163 s, so requests aborted while the backend was still working. The
  timeout is now 300 s — a safety net against stuck connections, not a
  patience limit (the request can always be cancelled).

### Changed

- The settings are now split into categories (UI-4, #30): General,
  Synchronization, AI assistant (filled by AI-A4, #12) and
  Maintenance live on separate pages of a Kirigami Addons
  `ConfigurationView` window with a category sidebar and search, replacing
  the single scrolling form window. Every setting keeps its wording and
  behavior; the sync page keeps its in-dialog status line and "Save and
  test sync" sequence. The `--test-settings-ui` hook now drives the sync
  sub-page directly, and a first settings screenshot ships in the README.
- Sidebar counts (system filters, projects, tags) now count open tasks
  only (`status != Completed`) instead of including completed history;
  recurring master tasks keep counting since they are never `Completed`.
  The "All" row shows `open/total` (e.g. `12/87`) so the total task count
  stays visible alongside the open count (UI-1, #27).
- The toolbar's Synchronize action is now hidden (and its shortcut
  `Ctrl+Shift+S` disabled) when no sync server is configured, instead of
  always being shown and silently falling back to a plain refresh
  (UI-2, #28).
- Quick capture: "Interpret with AI" is now a real button next to a short
  hint instead of a form list row. As a list row with a trailing chevron
  it read like a navigation target rather than an action (user report).

## [0.3.2] - 2026-07-17

### Fixed

- The Overdue sidebar row showed a generic placeholder icon:
  `appointment-missed` only ships as a 16px symbolic in Breeze. It now
  uses `data-warning` (user report).

## [0.3.1] - 2026-07-17

### Fixed

- Date fields in the form windows could not open their calendar popup
  (user report): FormCard's FormDateTimeDelegate anchors its popups via
  `ApplicationWindow.window`, which is null inside a plain `Window`. The
  form windows are now `QQC2.ApplicationWindow`s; the interaction test
  clicks the due-date row and asserts the popup opens.

## [0.3.0] - 2026-07-17

Team-built release ("full Taskwarrior manager"): a CLI capability
inventory, a taskchampion gap analysis and a UI/UX expert review were
produced by dedicated agents, then implemented in three waves.

### Added

- **Urgency** — the exact CLI formula (3.4.2 defaults) as sort order and
  in the detail window.
- **Start/stop** (active task) with context-menu action, "Active" chip,
  sidebar filter and `+ACTIVE` search.
- **Undo** (Ctrl+Z) — every mutation batch is one undoable step; like
  the CLI, undo cannot cross a sync.
- **until** expiry date in the detail window (CLI auto-delete semantics).
- **duplicate** context-menu action; "last modified" display; read-only
  UDA/foreign-attribute section in the detail window.
- **Search**: virtual tags (`+OVERDUE`, `+ACTIVE`, `+BLOCKED`, `+DUE`,
  `+TODAY`, `+WEEK`, `+TAGGED`, `+INSTANCE`, …), `due.before:`/
  `due.after:`, `project.not:`.
- **Date synonyms**: `sod`/`eod`/`sow`/`eow`/`soww`/`eoww`/`som`/`eom`/
  `soq`/`eoq`/`soy`/`eoy`, English weekday names, ordinals (`23rd`),
  `yesterday`, `now`, `later`/`someday`.
- **recur synonyms**: `weekdays`, `biweekly`/`fortnight`, `quarterly`,
  `semiannual`, `annual`/`biannual`, `Nwks`/`Nmo`/`Nqtr`/`Nyrs`.
- **JSON export** (task-export format incl. UDAs) from settings.
- **CLI coexistence guarantees**, verified end-to-end against the real
  `task` CLI on a shared sync server (`core/tests/cli_coexistence.rs`):
  UDAs survive app edits, CLI recurrence templates/instances are
  respected (no duplicate follow-ups), app-owned recur tasks are
  harmless for the CLI. See docs/architecture.md.

### Changed

- Priority chips use the accent color and localized labels — red is now
  reserved for overdue. Tags collapse to "+n" beyond two. Form windows
  gained breathing room; quick-capture moves notes below the structured
  fields. Arrow keys move the list selection (Shift extends).

### Known gaps (tracked as issues)

- CLI hooks never fire for app edits (library-level mutation).
- Month calendar/forecast (#1), detail column (#2), JSON import,
  contexts, further UI polish — see the issue tracker.

## [0.2.4] - 2026-07-17

### Fixed

- The sidebar scrollbar overlaid the count numbers: the sidebar content
  claimed the full drawer width instead of the scroll view's available
  width (user report with screenshot).

### Added

- The sidebar sections (Saved searches, Projects, Tags) collapse and
  expand by clicking their headers; the state is persisted. Project
  subtrees were already collapsible via their arrows.

## [0.2.3] - 2026-07-17

### Changed

- **Edit, quick-capture and settings are real dialog windows** now instead
  of modals overlaying the main window — movable, resizable, with a proper
  title bar (user feedback). The edit and quick-capture windows gained an
  inline error banner; settings keeps its existing sync status line, and
  the repair action now reports its error inline instead of pointing at
  the (hidden) main-view banner.
- The sync toolbar button only shows the blue activity dot when there
  actually are unsynchronized local changes; otherwise it shows a plain
  cloud. The dot was baked into the `state-sync` icon and appeared
  permanently before.

### Added

- The sidebar is resizable via a drag handle on its edge; the width is
  persisted across restarts (user report: counts were cut off).

## [0.2.2] - 2026-07-17

### Fixed

- The settings, detail, and quick-capture dialogs did not scroll: with
  more content than window height, the lower sections (sync fields,
  maintenance, dependencies, notes) were simply unreachable — user
  report with screenshot. The dialogs now use Kirigami.Dialog, which
  caps its height at the window and shows a scrollbar.
- Section headers in the rebuilt dialogs are left-aligned headings.

## [0.2.1] - 2026-07-17

### Fixed

- Saving sync settings gave no feedback inside the settings dialog: results
  and errors were reported via the main view's banner, which is hidden
  behind the modal dialog. The sync section now shows an inline status line
  ("Saved — last synchronized: …", "Synchronizing …", or the concrete error
  in red).
- Duplicate-binding warning for the search shortcut (StandardKey.Find).

### Added

- `--test-secrets` (bridge-level) and `--test-settings-ui` (real synthetic
  clicks + typing into the settings dialog, save button, persistence and
  reopen checks) — both verified against a live local sync server.

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

[Unreleased]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.4...v0.3.0
[0.2.4]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/hnsstrk/vergissmeinnicht-kde/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/hnsstrk/vergissmeinnicht-kde/releases/tag/v0.1.0
