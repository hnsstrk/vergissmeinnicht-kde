# Building

End-to-end build notes for Vergissmeinnicht (KDE). For the design rationale
read [`architecture.md`](architecture.md).

## Toolchain

- **Rust** (stable; developed against 1.97) — the whole app is a Cargo
  workspace, there is no CMake.
- **Qt 6** ≥ 6.6 (qt6-base, qt6-declarative) — developed against 6.11.
- **KDE Frameworks 6**: Kirigami, Kirigami Addons, ki18n, qqc2-desktop-style,
  Breeze icons (runtime QML dependencies; ki18n is also linked by a small
  C++ shim).
- **gettext** — `msgfmt` compiles the translation catalogs; `xgettext`
  regenerates the template.
- A C++ compiler — cxx-qt generates C++ glue that is compiled by `cc` from
  the build script.

Arch/CachyOS:

```sh
pacman -S --needed base-devel rust qt6-base qt6-declarative kirigami \
    kirigami-addons ki18n qqc2-desktop-style breeze-icons gettext
```

## Build & run

```sh
cargo build                  # Debug
cargo build --release        # Release
./target/debug/vergissmeinnicht
```

`scripts/install-local.sh` builds the release binary and installs everything
for the current user (`~/.local`): binary, desktop file, icon, AppStream
metainfo, and compiled translations. After that the app appears in the
application launcher.

## Test surface

```sh
cargo test --workspace
```

- `core` unit tests — timestamp/UUID/URL validation.
- `core/tests/replica_roundtrip.rs` — 20 integration tests against a real
  temporary replica (CRUD, metadata, annotations, dependencies,
  recurring follow-up, sync error paths). Port of the macOS Swift test
  suites.
- `app` unit tests — sidebar filter semantics, sorting, search operators,
  quick-capture/due/recur parsers, backup create/rotate/restore, state
  pipeline (36 tests).
- `cargo test -p vergissmeinnicht-app -- --ignored secrets` — real
  Secret Service roundtrip (needs an unlocked session with a running
  `org.freedesktop.secrets` provider).
- `cargo test -p vergissmeinnicht-app -- --ignored diktat --nocapture` —
  real dictation: records two seconds through `pw-record` (checks that the
  RIFF header is complete after the orderly stop) and runs the configured
  speech-to-text backend once. Needs PipeWire; the roundtrip also needs an
  installed backend. Everything else about dictation is covered by the
  regular unit tests (output parsers against recorded fixtures,
  availability probe, termination chain).

### End-to-end hooks

Every `--test-*` run refuses to start (exit 2, `TESTGUARD-FAIL`) unless both
`XDG_CONFIG_HOME` and `XDG_DATA_HOME` point away from the default locations:
the hooks write through the real save paths and have eaten the user's sync
server URL when run against the live config (#38). Always export both
variables to disposable directories. Note the guard cannot cover the Secret
Service — `--test-secrets` and `--test-settings-ui` intentionally exercise
the live KWallet/Secret Service and restore the previous entries afterwards.

```sh
# Scripted smoke test through the real QML→bridge chain (174 checks; parts
# whose prerequisites are missing — Secret Service, dictation chain — skip
# with a FLOW-INFO line and reduce the count).
# Use a disposable data dir — it mutates the replica!
# The AI sections (8+) only run with a configured model. To run the full
# flow without network or a local model, point VMN_AI_MOCK at the canned
# answers the flow expects and set the model in a throwaway config;
# VMN_STT_MOCK holds a canned transcript for the dictation→draft→commit
# section, which ends by creating a real task through the dialog's commit
# path (no microphone, no Whisper — without it that section skips with a
# FLOW-INFO line). This is exactly what the CI flow step does:
mkdir -p /tmp/vmn-test-cfg/vergissmeinnicht
printf '{"ai_model": "vmn-mock", "ai_base_url": "http://localhost:11434/v1"}' \
    > /tmp/vmn-test-cfg/vergissmeinnicht/config.json
VMN_AI_MOCK="$PWD/app/src/ai/fixtures/flow-konserven.json" \
VMN_STT_MOCK="Zahnarzttermin für nächste Woche ausmachen" \
XDG_DATA_HOME=/tmp/vmn-test XDG_CONFIG_HOME=/tmp/vmn-test-cfg \
    ./target/debug/vergissmeinnicht --test-flow

# The canned file is positional: entries 1–3 feed the AI worker section
# (stale-drop/cancel, hence the 800 ms delays), 4–5 the two interpret
# drafts, 6 the dictation draft (empty title — the transcript must survive
# in the title field), 7 the delayed control request of the state-machine
# counter-check. If you extend the AI flow sections in Main.qml, extend
# app/src/ai/fixtures/flow-konserven.json to match (format: module comment
# in app/src/ai/mock.rs). Note that saveAiSettings invalidates the cached
# client and a fresh mock starts over at entry 1 — sections that consume
# canned answers must run before the settings sections.
#
# VMN_STT_MOCK only pretends the PATH programs (pw-record, whisper) are
# installed and serves the transcript; backend-name, whisper.cpp path,
# start probe, and runtime-dir checks stay real, so the flow's negative
# probes keep working.

# Render the window (optionally with a dialog) into a PNG and quit:
XDG_DATA_HOME=/tmp/vmn-test XDG_CONFIG_HOME=/tmp/vmn-test-cfg \
    ./target/debug/vergissmeinnicht --test-dialog=detail --test-grab=/tmp/shot.png

# Regression guard for #38: opens the sync settings page, checks that the
# URL field mirrors the stored configuration (SYNCPAGE-…), and that saving
# keeps the URL instead of blanking it (SYNCSAVE-…):
XDG_DATA_HOME=/tmp/vmn-test XDG_CONFIG_HOME=/tmp/vmn-test-cfg \
    ./target/debug/vergissmeinnicht \
    --test-dialog=settings-sync --test-grab=/tmp/shot.png

# Interaction test: injects synthetic QMouseEvent/QKeyEvent into the window
# (click selection, Ctrl/Shift multi-selection, checkbox, double click,
# context menu, quick-capture typing). Needs the seeded demo dataset.
XDG_DATA_HOME=/tmp/vmn-test XDG_CONFIG_HOME=/tmp/vmn-test-cfg \
    ./target/debug/vergissmeinnicht --test-input
```

The synthetic events carry monotonically increasing fake timestamps — without
them QtQuick's delivery (Flickable, double-click detection) misbehaves; QTest
does the same internally.

### Sync end-to-end

`core/examples/sync_roundtrip.rs` verifies convergence of two replicas
through a real server:

```sh
# Terminal 1: a disposable local server
taskchampion-sync-server --listen 127.0.0.1:18080 --data-dir /tmp/tc-server

# Terminal 2:
cargo run -p vergissmeinnicht-core --example sync_roundtrip -- \
    http://127.0.0.1:18080 550e8400-e29b-41d4-a716-446655440000 some-secret
```

### Demo dataset for screenshots

```sh
# Seed tasks AND a demo AI config (English UI, Ollama base URL, recommended
# model). Base URL + model name are enough for `aiConfigured`, so the AI
# controls (microphone, "Interpret with AI", model combo) show up in
# screenshots without a running server, an API key, or a hand-built
# throwaway config. An existing config.json is never overwritten.
cargo run --release -p vergissmeinnicht-core --example seed_demo -- \
    /tmp/vmn-demo/vergissmeinnicht/replica --ai-config /tmp/vmn-demo-cfg

# The screenshot DoD wants the English locale; the config alone is not
# enough — ki18n also needs the compiled catalog in XDG_DATA_HOME:
mkdir -p /tmp/vmn-demo/locale/en/LC_MESSAGES
msgfmt -o /tmp/vmn-demo/locale/en/LC_MESSAGES/vergissmeinnicht.mo po/en.po

XDG_DATA_HOME=/tmp/vmn-demo XDG_CONFIG_HOME=/tmp/vmn-demo-cfg \
    ./target/debug/vergissmeinnicht

# Screenshots then go through the headless grab hooks. LANG covers the
# framework strings (e.g. the settings search field) that our catalog does
# not own; the three QT_* variables pin 1x rendering — on a scaled Wayland
# session the grab otherwise comes out at 1.5x and no longer matches the
# committed screenshots:
LANG=en_US.UTF-8 LANGUAGE=en \
QT_QPA_PLATFORM=xcb QT_ENABLE_HIGHDPI_SCALING=0 QT_FONT_DPI=96 \
XDG_DATA_HOME=/tmp/vmn-demo XDG_CONFIG_HOME=/tmp/vmn-demo-cfg \
    ./target/debug/vergissmeinnicht \
    --test-dialog=settings-ai --test-grab=docs/screenshots/settings-ai.png
```

Dialog names come from `testDialogTimer` in `app/qml/Main.qml` — note
`quickcapture`, not `quick-capture`.

One caveat for the AI page: the committed `settings-ai.png` shows a green
reachability line ("Backend reachable — 1 model found."), and that line does
need a reachable backend. The demo config is enough for `aiConfigured` and
therefore for the controls to appear, but on a machine without a local Ollama
the same recipe produces a screenshot with the unreachable state instead.
Start Ollama before grabbing that particular dialog, or expect the difference.

## Registration rules (the pbxproj of this repo)

Two places must be kept in sync by hand — forgetting either produces
confusing failures:

1. **New QML file** → add it to the `qml_files([...])` list in
   `app/build.rs`. Files missing there are not compiled into the QRC and
   imports fail at runtime only.
2. **New bridge Rust file** (with `#[cxx_qt::bridge]`) → add it to
   `.files([...])` in `app/build.rs`.

Also remember:

- Every method that QML calls needs `#[qinvokable]` **and** camelCase happens
  via the block-level `#[auto_cxx_name]` — a missing attribute surfaces at
  runtime as `Property 'x' of object AppContainer is not a function`.
- New user-visible strings: use `i18n(...)` (ki18n), then regenerate the
  template and update `po/en.po`:

  ```sh
  xgettext --from-code=UTF-8 -L JavaScript -ki18n:1 -ki18nc:1c,2 \
      -ki18np:1,2 -ki18ncp:1c,2,3 -o po/vergissmeinnicht.pot app/qml/*.qml
  msgmerge -U po/en.po po/vergissmeinnicht.pot
  ```

## Common failures

- **`Type X unavailable … Cannot assign to non-existent property`** at
  startup with no window: a QML file references an API that does not exist.
  Qt logs to the journal on Arch when stderr is not a console — run with
  `QT_FORCE_STDERR_LOGGING=1` to see engine errors.
- **`i18ndc is not defined`** from Kirigami Addons: the KLocalizedContext
  shim was not installed on the engine (see `app/cpp/klocalized.cpp`,
  called in `main.rs` before `engine.load`).
- **"Replica locked"** — a second instance is running against the same
  replica; SQLite enforces single-writer.
- **Linker error mentioning two `libsqlite3-sys` versions** — the `rusqlite`
  version in `app/Cargo.toml` must match the one taskchampion uses
  (see the comment there).
