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

- `cargo test --workspace` green; `cargo clippy --workspace -- -D warnings`
  clean.
- Functional changes: extend the `--test-flow` checks in `app/qml/Main.qml`
  when they touch bridge invokables, and run the flow against a disposable
  `XDG_DATA_HOME`.
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

## Verification without a desktop session

The repo has headless hooks (they also work while the session is locked):
`--test-flow` (scripted end-to-end through the real invokables),
`--test-dialog=<name>`, `--test-grab=<png>` (synchronous
`QQuickWindow::grabWindow`). Qt on Arch logs to journald when stderr is not
a console — use `QT_FORCE_STDERR_LOGGING=1` when hunting QML errors.
