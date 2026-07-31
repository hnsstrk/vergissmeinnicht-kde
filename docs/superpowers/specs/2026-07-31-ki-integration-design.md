# AI Integration Design — Vergissmeinnicht (KDE)

- **Date:** 2026-07-31
- **Status:** Approved design, pre-implementation
- **Scope:** Linux/KDE app only (`app/` layer). The shared core
  (`vergissmeinnicht-core`) stays untouched; macOS portability is
  explicitly not a requirement for this feature set.

## 1. Goal

Add opt-in AI assistance to Vergissmeinnicht: capturing tasks in natural
language (typed or dictated via Whisper), triaging the inbox, planning the
day, and chatting about the task store. The AI never mutates the store
itself — it fills forms and produces suggestion lists that the user applies
through the existing, user-confirmed code paths.

## 2. Decisions from the design interview

| Question | Decision |
|---|---|
| Use cases | NL capture (MVP, incl. dictation), inbox triage, day planner, chat — plus an ideas backlog (§9) |
| Model backend | Configurable: any OpenAI-compatible endpoint, via provider presets — **Ollama** (local, default), **OpenRouter** (cloud, incl. Claude models as `anthropic/claude-*`), or a custom URL. API key per provider |
| Autonomy | Suggest only, never execute |
| Layer | KDE app only (`app/src/ai/`), no core changes |
| Realization | In-app, Rust-native — no sidecar process, no MCP server |
| Speech-to-text | Selectable local backend: the `openai-whisper` CLI (CPU) or a `whisper.cpp` build (GPU), both invoked as subprocesses |

## 3. Guiding principles

1. **Suggest, never execute.** Every AI output lands as a draft or a
   suggestion list. Store mutations always go through the existing
   user-confirmed invokables; the AI has no write path of its own.
2. **Strictly optional.** Without a configured backend the app behaves
   exactly as today; all AI controls are hidden behind an `aiConfigured`
   property (analogous to `sync_configured`).
3. **Data sovereignty is visible.** The default endpoint is localhost —
   nothing leaves the machine. When a non-local endpoint is configured,
   the settings page states explicitly that task data is transmitted to it.
4. **No shared state with sync.** `error_message` is a single global slot
   shared by sync and mutations; the AI gets its own `aiBusy` / `aiError`
   properties and never writes the global one.

## 4. Architecture

### 4.1 New module `app/src/ai/`

Qt-free and unit-testable, like `parsers.rs`.

- `client.rs` — `LlmClient`: blocking `reqwest` client against any
  OpenAI-compatible `/v1/chat/completions` endpoint. `reqwest 0.12` is
  already resolved in the tree (transitively via taskchampion), so this
  adds no second TLS stack. JSON output is enforced on two levels: the
  request asks for it (`response_format: {"type": "json_object"}` —
  honored by Ollama and most OpenRouter models) **and** the prompt demands
  it, because some endpoints silently ignore `response_format` (Anthropic's
  OpenAI-compatibility layer documents exactly that). Every response is
  validated regardless of what the endpoint promised. A trait `Llm` fronts
  the client so tests can substitute a mock. On unparseable output: one
  silent retry with a format reminder appended, then error.
- `transcribe.rs` — `Transcriber`: records via `pw-record` (PipeWire,
  present on every current Linux desktop — avoids a new qt6-multimedia
  dependency), then transcribes through one of two selectable backends,
  both plain subprocesses with backend-specific output parsing:
  - **`openai-whisper`** (default): the `whisper` CLI from PATH
    (`whisper --model <m> --language de --output_format json
    --output_dir <runtime-dir>`). Runs on CPU; models auto-download on
    first use.
  - **`whisper.cpp`**: a user-provided `whisper-cli` binary plus a GGML
    model file (both configured as paths in the settings) —
    `whisper-cli -m <model> -f <wav> -l de --output-json`. This covers
    GPU builds; the reference machine has a ROCm/HIP build with
    `ggml-large-v3` from the RPG-audio project.

  Availability is probed once at startup: `pw-record` plus the configured
  backend's binary (and model file, for whisper.cpp). If anything is
  missing, the microphone button is not shown at all. Recordings and
  transcripts live in the XDG runtime dir and are deleted after use.
- `types.rs` — suggestion data types (`AiDraft`, `TriageSuggestion`,
  `PlanEntry`, `ChatMessage`), serde-serializable; they cross the bridge
  as JSON QStrings (the app's established pattern).
- `prompts.rs` — prompt builders per feature. Every prompt that resolves
  dates includes the current date/time; every prompt that assigns
  projects/tags includes the user's existing project and tag names so the
  model maps into the existing taxonomy instead of inventing categories.

### 4.2 Bridge integration

- Invokables stay on the existing `AppContainer` (it owns the state
  snapshot access); they are thin wrappers that delegate to the `ai`
  module. Implementation lives in `ai/`, not in `bridge.rs`.
- New properties: `aiConfigured`, `aiBusy`, `aiError`, and
  `dictationAvailable` are base infrastructure (they ship with stage 1
  and serve every stage). Per-stage result properties follow:
  `aiDraftJson` (stage 1), `aiSuggestionsJson` (stage 2), `aiPlanJson`
  (stage 3), `chatMessagesJson` (stage 4).
- **Threading** follows the proven `start_sync` pattern: clone the needed
  task snapshot, `std::thread::spawn`, blocking HTTP call in the worker,
  result marshalled back via `qt_thread().queue(...)`. AI results are
  published as plain property sets — never through `apply()`, which would
  force a full model reset.
- **Cancellation:** a generation counter. Each request stores its
  generation; “Cancel” (and any newer request) bumps the counter; stale
  results are dropped in the queued callback. The HTTP client carries a
  fixed 120 s timeout (local models need load time on first use).
- **No `#[qsignal]`, no streaming in v1.** Chat answers appear complete.
  Token streaming would introduce the project's first signal and is
  deferred until chat UX feedback justifies it.

### 4.3 Settings and secrets

- New `Settings` fields (backwards compatible via `#[serde(default)]`):
  `ai_provider` (preset: `ollama` | `openrouter` | `custom`; default
  `ollama`), `ai_base_url` (prefilled by the preset:
  `http://localhost:11434/v1` for Ollama,
  `https://openrouter.ai/api/v1` for OpenRouter; editable), `ai_model`
  (default empty), `ai_stt_backend` (`openai-whisper` | `whisper-cpp`;
  default `openai-whisper`), `ai_whisper_model` (default `small`, for the
  openai-whisper backend), `ai_whisper_cpp_binary` and
  `ai_whisper_cpp_model` (paths, for the whisper.cpp backend).
  `aiConfigured` requires a non-empty base URL and model name, cached like
  `compute_sync_configured`.
- **Claude models** are reached through OpenRouter (`anthropic/claude-*`)
  — that is the recommended path, since it speaks the same
  OpenAI-compatible protocol as everything else. Anthropic's own
  OpenAI-compatibility endpoint (`https://api.anthropic.com/v1/`) works
  via the `custom` preset, but Anthropic documents it as a
  testing-oriented layer that ignores `response_format`; the prompt-level
  JSON enforcement in §4.1 covers that case.
- The API key (needed for cloud endpoints only) is stored in the Secret
  Service via `secrets.rs`. The existing service string is literally
  `de.hnsstrk.vergissmeinnicht.sync`; the AI key gets its own service
  string `de.hnsstrk.vergissmeinnicht.ai` rather than squatting in the
  sync-named service. It is never written to `config.json`.
- Settings UI: a new **“AI assistant”** section modeled 1:1 on the sync
  section — explanatory row, provider combo box (prefills the base URL),
  base URL field, model name field, API key password field, a
  speech-to-text backend combo with its per-backend fields (whisper model
  combo, or binary/model path fields), and a **“Save and test”** button
  that performs a cheap request (model list) and reports the result in
  the section's own status line. A privacy note appears when the base URL
  is not localhost.

## 5. Feature stages

Each stage is independently shippable; stage 1 carries the whole
infrastructure of §4. The implementation plan is written **per stage**
(stage 1 including the §4 infrastructure first), not as one plan across
all four — each stage triggers the full repo DoD (screenshots,
CHANGELOG, both READMEs, po catalogs, help dialog) on its own.

### Stage 1 — MVP: dictation + natural-language capture

Quick Capture gains two controls, visible only when `aiConfigured`:

- **Microphone button** (visible only when `dictationAvailable`): click
  starts `pw-record` (button pulses), second click stops it. Whisper
  transcribes (spinner via `aiBusy`), the transcript lands in the title
  field and **automatically continues into interpretation** — dictation
  and parsing are one flow.
- **“Interpret with AI”** (shortcut Ctrl+J): sends the free text to the
  model. Response schema:
  `{title, project, tags, due, priority, recur, notes}`.

The result is written **into the structured form fields** of the dialog —
not only the preview row. The user reviews and corrects each field, then
commits as usual; the commit path is the existing `addTaskDetailed`
invokable, unchanged. `due` values are validated with `parse_due_token`,
`recur` with `is_valid_recur_token`; anything invalid leaves the field
empty rather than inserting garbage. The dialog's due/recur combo boxes
currently offer only five presets each; valid AI values outside the
presets (e.g. `+3d`, an ISO date, `quarterly`) land via the custom-value
pattern already established in `DetailDialog.qml` (custom text field
shown when the combo is set to “custom”, date picker fill for `due`) —
Quick Capture adopts that pattern as part of this stage. The model may propose a project name
that does not exist yet — it appears in the form like any typed value and
creates the project on commit, exactly as manual input would.

Both speech-to-text backends reload their model on every CLI invocation
(seconds — more for whisper.cpp with `large-v3`); for short dictations
this is acceptable in v1. The openai-whisper backend runs CPU-only on the
reference machine (PyTorch without ROCm); the whisper.cpp backend uses
the GPU and delivers the best German quality via `large-v3`.

### Stage 2 — Inbox triage

A new action in the Inbox perspective: **“Sort inbox with AI”**. One
request carries all inbox tasks (chunked at 40 tasks per call) plus the
project/tag taxonomy. Response per task: suggested project, tags,
priority, and a one-sentence rationale.

Suggestions open in their own window (`FormWindow`): one row per task —
title, suggestion, rationale, and a checkbox (checked by default).
“Apply selected” walks the accepted rows through the existing mutation
path in one pass (mirroring `bulk_apply`), yielding one undoable step;
“Dismiss” closes without consequence.

### Stage 3 — Day planner

Action **“Plan my day”** in the Today perspective. Prompt context: today's,
overdue, due-soon, and started tasks as compact JSON (title, project, due,
priority, urgency, tags). These subsets are selected through the existing
`SidebarFilter::matches` — per the repo's architecture invariant, filter
logic is never duplicated. Response:

1. **An ordered day plan** (3–7 tasks with a short rationale) — pure
   advice, changes nothing.
2. **Breakdown suggestions** for tasks that look too large: sub-steps as
   creatable tasks (chained via `depends`), each with a “Create” button.
3. **Deferral hints** for visibly stale tasks: a realistic new due date,
   acceptable per row.

All in a dedicated “Day plan” window; only 2. and 3. touch the store, and
only through existing invokables.

### Stage 4 — Chat about the store

A chat window (toolbar button). History lives in `chatMessagesJson` for
the session only — nothing is persisted. Context: a compact snapshot of
pending tasks (uuid, title, project, tags, due, status, urgency), plus
recently completed tasks for review-style questions.

**No tool-calling in v1.** The model answers with structured JSON:
`{answer, suggestions[]}`. Questions are answered in `answer`; proposed
changes appear as **suggestion cards with an apply button**, reusing the
triage suggestion types and apply path. The suggest-only guarantee is
thereby structural, not behavioral — and it works identically with small
local models and cloud models.

## 6. Data sent to the model, per feature

| Feature | Data in the prompt |
|---|---|
| Capture (stage 1) | The raw input text, current date/time, project names, tag names |
| Triage (stage 2) | Inbox tasks (title, existing tags, notes), project and tag names |
| Planner (stage 3) | Today/overdue/due-soon/started tasks: title, project, due, priority, urgency, tags |
| Chat (stage 4) | The user's messages, compact pending snapshot, recently completed tasks |

Nothing else is transmitted. No telemetry. AI outputs are never persisted;
the chat history dies with its window. Audio recordings and transcripts
are deleted after transcription. The API key lives in the Secret Service.

## 7. Error handling

- `aiError` is displayed at the surface that triggered the action
  (capture dialog, triage window, chat) — never in the global banner.
- Unreachable backend → a clear, provider-aware message (“AI backend
  unreachable — is Ollama running?” for the Ollama preset, a generic
  network message otherwise); the settings “Save and test” button catches
  misconfiguration early.
- Model output that fails validation is retried once with a format
  reminder, then reported as an error. Only validated data reaches the UI.
- Missing `pw-record`/`whisper` → the microphone button is hidden, not
  broken.
- A hanging request never blocks the app: cancellation via generation
  counter, plus the 120 s client timeout.

## 8. Testing strategy

- The `ai` module is Qt-free: unit tests for prompt building and response
  validation (malformed JSON, invalid `due`, unknown fields).
- The `Llm` trait gets a mock implementation for app-logic tests without
  network.
- `--test-flow` is extended with a mock hook (environment variable
  pointing at canned responses) so the full path dictation → draft →
  commit is verifiable headless, including the `I18N_ARGUMENT_MISSING`
  grep required by the repo's DoD.
- Standard repo duties apply: new strings via `i18n(...)` + pot/`en.po`
  regeneration, new shortcuts in `HelpDialog.qml`, screenshots of new
  windows with the seeded demo dataset, `CHANGELOG.md` + both READMEs,
  `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.

## 9. Ideas backlog (documented, not planned)

Deliberately out of the staged plan; revisit only on demand:

1. **Semantic search & duplicate warning** — embedding index over titles
   and annotations via a local embedding model (Ollama `/api/embed`,
   e.g. `snowflake-arctic-embed2`); meaning-based search and an “a similar
   task already exists” hint during capture. Fully local, no LLM needed.
2. **Natural-language search** — translate free text into the existing
   operator search (`project:… due.before:…`), showing the generated
   query transparently.
3. **Weekly review report** — a summary built from tasks completed and
   created during the week.
4. **Recurrence detection** — notice repeatedly hand-created similar
   tasks and suggest converting them into a recurring task.
5. **Notification digest** — phrase the existing overdue notification as
   a prioritized one-line summary instead of a raw count.

## 10. Out of scope / deferred

- **Token streaming** (`#[qsignal]`) — deferred until chat UX feedback
  justifies the project's first signal.
- **Tool-calling in chat** — the structured suggest-only schema replaces
  it in v1.
- **Core/macOS portability** — decided against for this feature set.
- **WhisperX-ROCm as a third STT backend** — planned as the primary
  engine in the RPG-audio project, but not installed on the reference
  machine (requires a self-built ROCm CTranslate2 stack). The backend
  seam in §4.1 keeps the door open.
- **A native Anthropic Messages-API client** — Claude models are served
  through the OpenAI-compatible path (§4.3); a second, native protocol
  client is not worth the surface area for v1.

## Reference environment (Ganymed, 2026-07-31)

Ollama 0.32 with ROCm (RX 7900 XTX): `qwen3.6:27b-128k`,
`gemma4:26b-256k`, `ministral-3`, `granite4.1:8b`, embedding model
`snowflake-arctic-embed2`. `python-openai-whisper` 20250625 (CLI
`/usr/bin/whisper`), currently with the `base` model cached, PyTorch
CPU-only. whisper.cpp ROCm/HIP build with `ggml-large-v3.bin` at
`~/Projekte/rpg-audio-studio/models/whisper.cpp/` (binary
`build/bin/whisper-cli`), verified on gfx1100 by the RPG-audio project.
WhisperX-ROCm is not installed. PipeWire (`pw-record`) available.
