# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

Vergissmeinnicht (KDE) — a native Kirigami client for Taskwarrior 3.x on top
of TaskChampion. Rust workspace, no CMake: `core/` wraps taskchampion
(`TaskStore`/`TaskInfo`/`VmError`), `app/` holds the cxx-qt bridge, app
logic, and the QML UI. This is the Linux port of
<https://github.com/hnsstrk/vergissmeinnicht> (macOS); the core API is kept
deliberately identical between the ports so fixes can travel.

## Language conventions

- **Code comments and QML source strings: German** (source language of the
  UI; English lives in `po/en.po`).
- **GitHub-facing documentation (`README.md`, `docs/`): English**;
  `README.de.md` is the intentional German counterpart.
- **Commit messages: English.**

## Build pipeline duties

- New QML file → register in `qml_files([...])` in `app/build.rs`.
- New bridge Rust file → register in `.files([...])` in `app/build.rs`.
- QML-callable methods need `#[qinvokable]`; camelCase comes from the
  block-level `#[auto_cxx_name]`. A missing attribute fails only at runtime
  ("… is not a function").
- New UI strings use `i18n(...)`; regenerate `po/vergissmeinnicht.pot` with
  xgettext and update `po/en.po` in the same change (see
  `docs/building.md`).
- `rusqlite` in `app/Cargo.toml` must match taskchampion's version (single
  `libsqlite3-sys` in the tree).

## Definition of Done — user-facing changes

- `cargo test --workspace` green; `cargo clippy --workspace --all-targets
  -- -D warnings` clean (`--all-targets` covers test and example code).
- Functional changes: extend the `--test-flow` checks in `app/qml/Main.qml`
  when they touch bridge invokables — never delete existing checks when
  resolving a conflict there — and run the flow against disposable
  `XDG_DATA_HOME`/`XDG_CONFIG_HOME`. Grep the flow log for
  `I18N_ARGUMENT_MISSING` and `kf.i18n:.*instead of` — ki18n reports missing
  `%1` arguments only at runtime (`i18n("…%1…", x)`, never `.arg()`
  chaining!).
- Visible window changes: refresh `docs/screenshots/` via
  `--test-dialog=… --test-grab=…` with the seeded demo dataset
  (`core/examples/seed_demo.rs`) and English locale.
- Update `CHANGELOG.md` (Unreleased section), `README.md` **and**
  `README.de.md` in the same commit.
- New shortcuts must appear in the help dialog (`HelpDialog.qml`).

## Architecture invariants

- `AppState` (Rust) is the single source of truth; QML holds only view
  state (selection, dialog fields). All mutations run through
  `AppContainer::apply` → model reset + property publish + error report.
- Sidebar counts and the visible list share `SidebarFilter::matches` —
  never duplicate filter logic.
- The stable task identifier is the UUID; the working-set id is display
  only.
- The replica in `~/.local/share/vergissmeinnicht/` is app-owned; never
  point the app at the Taskwarrior CLI's data directory. Data exchange goes
  through the sync server exclusively.
- Sync credentials live in the Secret Service, never in the config file.
- **CLI recurrence is sacred**: never write `parent`/`imask`/`mask`/`rtype`,
  never create follow-up instances for tasks that carry `parent`/`imask`
  (`TaskInfo::is_recurring_child`), never complete `status:recurring`
  templates. The app's own follow-up model applies only to app-created
  recur tasks (plain pending + `recur`). Verified end-to-end by
  `core/tests/cli_coexistence.rs` (run with a local sync server and the
  `task` CLI: `cargo test -p vergissmeinnicht-core --test cli_coexistence
  -- --ignored`).
- Every core mutation batch starts with an `Operation::UndoPoint`
  (`mutation_ops()`) — one batch = one undoable step.

## Verification without a desktop session

The repo has headless hooks (they also work while the session is locked):
`--test-flow` (scripted end-to-end through the real invokables),
`--test-dialog=<name>`, `--test-grab=<png>` (synchronous
`QQuickWindow::grabWindow`). Qt on Arch logs to journald when stderr is not
a console — use `QT_FORCE_STDERR_LOGGING=1` when hunting QML errors.

Before every measured run:

- **Rebuild, or you measure the wrong binary.** The QML lives in the QRC
  inside the binary, and `cargo build` does not reliably pick up QML edits:
  `touch app/qml/Main.qml && cargo build`, then measure.
- **Redirect both XDG variables** — `XDG_DATA_HOME` **and**
  `XDG_CONFIG_HOME`. Otherwise the guard aborts the run with
  `TESTGUARD-FAIL`, and rightly so: test runs have destroyed the user's real
  config. Ready-made command lines are in `docs/building.md`.

For screenshots, the valid dialog names are listed in the `testDialogTimer`
in `app/qml/Main.qml`; the committed images live in `docs/screenshots/`. The
recipe for English screenshots with the demo dataset is in
`docs/building.md`.

## Patterns to reuse

Reach for an existing pattern before inventing one:

- Threading and long-running work: `start_sync` in `app/src/bridge.rs`.
- Synchronous parse/preview calls from QML: `quick_capture_preview_json`
  (same file).
- Batch mutations: `bulk_apply`.
- New settings: `config.rs` + `secrets.rs` + the matching page under
  `app/qml/*SettingsPage.qml`.

## Known pitfalls

- Dialogs on settings pages need `parent: page.QQC2.Overlay.overlay`,
  otherwise they open behind the window (`MaintenanceSettingsPage.qml`).
- ComboBox popups need a height cap — use `VmComboBoxDelegate`; without it
  a long model covers the entire settings page.
- Never `git add -A` after `msgmerge`: it stages the `po/en.po~` backup.

## UI text and interaction

- Source language is German; English is written in `po/en.po`.
- Say what happens, not what the software can do. No advertising, no
  exclamation marks. Technical terms only where the context carries them.
- KDE convention for actions: infinitive; ellipsis only when a further
  dialog follows.
- For interaction, Plasma conventions and the KDE HIG beat the more
  original solution — familiarity is a usability gain. Deviate only with a
  reason.

## Questions about third-party APIs

The installed source beats any web page. Look there first and note the
version — the answer only holds for that version.

- Kirigami: `/usr/lib/qt6/qml/org/kde/kirigami/`; Kirigami Addons:
  `/usr/lib/qt6/qml/org/kde/kirigamiaddons/`.
- cxx-qt and taskchampion: `~/.cargo/registry/src/…`.
- KDE beyond that: api.kde.org and invent.kde.org.
