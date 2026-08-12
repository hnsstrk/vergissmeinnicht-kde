# Vergissmeinnicht (KDE)

[![CI](https://github.com/hnsstrk/vergissmeinnicht-kde/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/hnsstrk/vergissmeinnicht-kde/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/hnsstrk/vergissmeinnicht-kde?sort=semver)](https://github.com/hnsstrk/vergissmeinnicht-kde/releases/latest)
[![License: MIT](https://img.shields.io/github/license/hnsstrk/vergissmeinnicht-kde)](LICENSE)

Ein nativer KDE-Plasma-Client für [Taskwarrior](https://taskwarrior.org) 3.x
auf Basis von [TaskChampion](https://github.com/GothenburgBitFactory/taskchampion).
Kirigami-Oberfläche, Rust-Kern via [cxx-qt](https://github.com/KDAB/cxx-qt).

Dies ist der Linux/KDE-Port der
[gleichnamigen macOS-App](https://github.com/hnsstrk/vergissmeinnicht) —
derselbe Rust-Kern, dieselbe Replica-plus-Sync-Architektur, native
Oberfläche auf jeder Plattform.

🇬🇧 [English version](README.md)

![Vergissmeinnicht — Ansicht „Zu erledigen“](docs/screenshots/main.png)

## Funktionen

- **Seitenleisten-Perspektiven** — Eingang · Heute · Zu erledigen · Überfällig ·
  Bald fällig · Geplant · Wartend · Alle, dazu Projekt- und Tag-Zeilen mit
  Live-Zählern offener Aufgaben (Wiedervorlage-Master zählen mit, Erledigte
  nicht); die Alle-Zeile zeigt `offen/gesamt`. Dazu Drop-Ziele und
  Kontextmenüs (Umbenennen/Entfernen). Gepunktete Projekte
  (`Arbeit.Teilprojekt`) bilden einen klappbaren Baum;
  die Auswahl eines Elternprojekts schließt Subprojekte ein
  (Taskwarrior-Präfix-Semantik). Die Breite lässt sich per Zieh-Griff
  anpassen, Sektionen klappen per Klick auf ihre Überschrift ein und aus;
  beides bleibt gespeichert.
- **Volltextsuche mit Operatoren** (Strg+F) — durchsucht Titel, Projekt, Tags
  und Notizen über den gesamten Bestand (offen, erledigt, wiederkehrend).
  UND-Verknüpfung, Phrasen in Anführungszeichen sowie `projekt:`, `tag:`,
  `status:` (deutsche und englische Aliase). Bei aktiver Suche wird der
  Seitenleisten-Filter ignoriert. Freitext ist tippfehlertolerant: deutsche
  Schreibvarianten werden gefaltet (`pruefen` findet „prüfen", `strasse`
  findet „Straße"), eine kleine Editierdistanz fängt Vertipper wie `prüfem`
  ab. Wörter mit höchstens drei Buchstaben müssen exakt stimmen, die
  Operatoren oben bleiben ebenfalls exakt.
- **Gespeicherte Suchen** (Strg+Umschalt+D) — Suche benennen und in der
  Seitenleiste anheften. Rechtsklick zum Umbenennen oder Löschen.
- **Schnelleingabe** (Strg+N) — Fenster mit Titel, Notizen, Projekt, Tags,
  Fälligkeit, Priorität, Wiederholung. Das Titelfeld versteht
  Terminal-Syntax (`+tag project:foo due:tomorrow priority:H`) mit
  Live-Vorschau. Wie Detail-Editor und Einstellungen öffnet sie sich als
  eigenständiges Dialogfenster (frei beweg- und skalierbar), nicht als
  Modal im Hauptfenster. Mit konfiguriertem KI-Backend schickt **Mit KI
  interpretieren** (Strg+J) den Freitext des Titels ans Modell und füllt
  die strukturierten Felder aus der validierten Antwort — ungültige
  Fälligkeits-/Wiederholungs-/Prioritätswerte werden verworfen, neue
  Projektnamen sind erlaubt, Felder, die das Modell leer lässt, behalten
  die eingegebenen Werte (ersetzt wird nur, was das Modell wirklich
  füllt), und angelegt wird erst über den normalen Hinzufügen-Knopf. Mit
  vollständiger Diktier-Kette (siehe KI-Assistent unten) — sie ist vom
  konfigurierten Modell unabhängig, Diktieren funktioniert also auch auf
  Maschinen, die kein Sprachmodell stemmen — nimmt der Mikrofonknopf
  daneben beim ersten Klick auf (der Knopf pulsiert), der zweite beendet
  und transkribiert — das Transkript landet im Titelfeld und läuft
  automatisch in die KI-Interpretation weiter. Was von den beiden
  Strängen nicht eingerichtet ist, erscheint als gesperrter Knopf mit dem
  Grund im Tooltip, statt versteckt zu werden. Aufnahme und Transkription
  lassen sich jederzeit verwerfen; Fehler erscheinen im Statusbereich des
  Dialogs.
- **Detail-Editor** — Titel, Projekt, Tags, Fällig, Geplant ab, Warten bis,
  Priorität, Wiederholung, Notizen, Abhängigkeits-Editor, Reaktivieren
  erledigter Aufgaben.
- **Mehrfachauswahl** mit Sammel-Aktionen (Erledigt / Löschen / Projekt /
  Tag / Priorität / Fälligkeit / Zurückstellen) über das Kontextmenü
  (Strg/Umschalt+Klick, Strg+A).
- **Drag & Drop** von Aufgaben auf Projekte, Tags oder den Eingang
  (entfernt Projekt + Tags).
- **Wiederkehrende Aufgaben** — täglich / wöchentlich / monatlich / jährlich
  sowie `Nd / Nw / Nm / Ny`. Das Erledigen erzeugt atomar die Folge-Instanz.
- **Zurückstellen (Snooze)** — verschobene Aufgaben erscheinen unter
  „Wartend“ statt „Heute“ zu verstopfen.
- **Abhängigkeiten** — Berichte Blockiert / Blockierend / Nicht blockiert
  (`+BLOCKED`/`+BLOCKING`/`+UNBLOCKED`-Semantik) plus Abhängigkeits-Editor im
  Detail-Dialog (`depends`-Relationen hinzufügen/entfernen, mit Titel-Auflösung).
- **Benachrichtigungen** — Opt-in-Zusammenfassung beim Start, wenn
  überfällige Aufgaben vorliegen.
- **Einstellungen in Kategorien** — Allgemein, Synchronisation,
  KI-Assistent und Wartung als eigene Seiten in einem Einstellungsfenster
  mit Kategorien-Seitenleiste.

  ![Vergissmeinnicht — Einstellungen](docs/screenshots/settings.png)
- **KI-Assistent (opt-in)** — konfiguriert unter Einstellungen →
  KI-Assistent: Provider-Preset (Standard Ollama lokal, OpenRouter oder
  ein eigener OpenAI-kompatibler Endpunkt), Modellauswahl aus der
  Modellliste des Endpunkts (wird beim Öffnen der Seite automatisch
  geladen; eine Erreichbarkeitszeile zeigt, ob der Endpunkt antwortet und
  wie viele Modelle er anbietet, „Modelle laden" bleibt als manuelle
  Aktualisierung — manuelle Eingabe bleibt auch offline möglich; ein
  Provider-Wechsel oder eine geänderte Basis-URL verwirft die veraltete
  Liste samt Erreichbarkeitsanzeige und lädt still gegen den neuen
  Endpunkt nach — ohne den gespeicherten API-Schlüssel mitzusenden, der
  zum gespeicherten Endpunkt gehört), API-Key
  im Secret Service und das Spracherkennungs-Backend fürs Diktat — entweder
  `openai-whisper` (das `whisper`-Programm aus dem `PATH`, CPU, Modellname
  einstellbar) oder `whisper.cpp` (ein `whisper-cli`-Programm plus
  GGML-Modelldatei, beides als Pfad; so lässt sich ein GPU-Build nutzen).
  Bei `whisper.cpp` wird das Verzeichnis des Programms dem
  `LD_LIBRARY_PATH` des Kindprozesses vorangestellt — ROCm-/HIP-Builds
  legen `libwhisper.so` neben das Programm, der bloße Binärpfad genügt
  also ohne Wrapper-Skript — und die Verfügbarkeitssonde startet das
  Programm einmal (`--help`): Ein Programm, das seine Bibliotheken nicht
  findet, gilt als nicht verfügbar. Zusätzlich braucht das Diktat
  `pw-record` aus PipeWire; fehlt ein Glied dieser Kette, bleibt das
  Mikrofon sichtbar, aber gesperrt — sein Tooltip nennt das fehlende
  Glied —, statt erst bei der Aufnahme zu scheitern.
  Aufnahmen und Transkripte liegen im XDG-Laufzeitverzeichnis und werden
  nach Gebrauch gelöscht. Der Kontextumfang bestimmt, wie viele
  Aufgabendaten die KI beim Interpretieren sieht: nur Projekt- und
  Schlagwortnamen (Standard), zusätzlich Titel offener Aufgaben oder alle
  nicht gelöschten Aufgaben kompakt — Gelöschtes wird nie übertragen.
  „Speichern und testen" prüft die Verbindung über die Modellliste; bei
  nicht-lokalen Endpunkten erscheint ein Datenschutzhinweis.

  ![Vergissmeinnicht — KI-Einstellungen](docs/screenshots/settings-ai.png)

  **Welches Modell.** Gemessen auf der Referenzmaschine (lokales Ollama,
  Radeon RX 7900 XTX, 64k Kontext) lieferte **`gemma4:12b` die besten
  Ergebnisse** und ist die Empfehlung: rund 20 s je Interpretation, und in
  8 Läufen wurde jede Eingabe einem vorhandenen Projekt mit korrekter
  Fälligkeit zugeordnet. Reasoning-Modelle sind für diese Aufgabe deutlich
  schlechter — `qwen3.6:27b` brauchte für dasselbe 90–163 s und ließ das
  Projektfeld in etwa der Hälfte der Läufe leer. Das Denken lässt sich über
  den OpenAI-kompatiblen Endpunkt nicht abschalten (`think: false` gibt es
  nur in Ollamas eigener `/api/chat`, und `reasoning_effort` schaltet es in
  jeder Stufe *ein*) — der Hebel ist also die Modellwahl. Der Anfrage-
  Timeout liegt bei 300 s, damit auch langsame Backends durchkommen.
- **Lokalisierung** — Deutsch (Quellsprache) und Englisch über
  ki18n/gettext, mit manueller Umschaltung in den Einstellungen.
- **Synchronisierung** gegen einen beliebigen
  [taskchampion-sync-server](https://github.com/GothenburgBitFactory/taskchampion-sync-server).
  Zugangsdaten liegen im Secret Service des Systems (KWallet).
  Auto-Sync: manuell, alle 5/15/60 Minuten oder sofort nach Änderungen.
  Der Werkzeugleisten-Knopf zeigt unsynchronisierte lokale Änderungen mit
  einem blauen Punkt an und ist (samt Tastenkürzel Strg+Umschalt+S)
  ausgeblendet, solange kein Sync-Server konfiguriert ist.
- **Automatische Backups** — `VACUUM INTO`-Snapshot vor jedem Sync,
  rotierend die letzten 10. Manuelles Backup und Wiederherstellung in den
  Einstellungen. Siehe [`docs/backup-and-restore.md`](docs/backup-and-restore.md).
- **Erledigte aufräumen** — eine Wartungsaktion löscht erledigte
  Aufgaben ab einem wählbaren Mindestalter (1 Monat bis 1 Jahr, nach
  letzter Änderung). Die Bestätigung nennt die exakte Anzahl und friert
  die betroffene Menge ein — gelöscht wird nie mehr als bestätigt, egal
  wie lange der Dialog offen steht. Vorher wird automatisch ein Backup
  angelegt, der gesamte Lauf ist ein einziger Undo-Schritt (Strg+Z), und
  CLI-verwaltete Wiederholungen bleiben unangetastet.

  ![Vergissmeinnicht — Wartungs-Einstellungen](docs/screenshots/settings-maintenance.png)
- **Taskwarrior-Parität** — Dringlichkeit (exakte CLI-Formel) als
  Sortierung, Start/Stopp (aktive Aufgabe), Rückgängig (Strg+Z),
  `until`-Ablaufdatum, Duplizieren, JSON-Export inkl. UDAs, virtuelle
  Tags und `due.before:`/`due.after:`/`project.not:` in der Suche,
  CLI-Datums-Synonyme (`eow`, `friday`, `23rd`, …) und
  recur-Synonyme (`weekdays`, `quarterly`, …). CLI-Recurrence-Vorlagen
  werden respektiert, nie dupliziert — die Koexistenz mit der
  `task`-CLI am gemeinsamen Sync-Server ist Ende-zu-Ende verifiziert
  (siehe `docs/architecture.md`).
- **Legacy-Reparatur** — eine Wartungsaktion überführt Token-Syntax in
  Aufgabentiteln (`+tag project:x`) in echte Eigenschaften.

*(Alle Screenshots zeigen einen deterministischen Demo-Datensatz —
`cargo run --release -p vergissmeinnicht-core --example seed_demo -- <replica-pfad>`.
Screenshots mit KI-Bedienelementen nutzen zusätzlich die Demo-KI-Konfiguration,
die dasselbe Beispiel mit `--ai-config` schreibt — ohne Server und ohne
API-Key, siehe `docs/building.md`.)*

## Architektur

```
┌─────────────────────────────────────────────┐
│  Kirigami/QML-UI (Hauptfenster + Dialoge)   │
│  Sidebar · Taskliste · Detail · Settings    │
└──────────────────┬──────────────────────────┘
                   │  cxx-qt-Bridge (QAbstractListModel + Invokables)
┌──────────────────▼──────────────────────────┐
│  vergissmeinnicht-app (Rust)                │
│  AppState · Filter · Parser · Backups       │
└──────────────────┬──────────────────────────┘
                   │  reines Rust
┌──────────────────▼──────────────────────────┐
│  vergissmeinnicht-core (Rust)               │
│  taskchampion 3.x · tokio                   │
│  Replica = SQLite im XDG-Datenverzeichnis   │
└──────────────────┬──────────────────────────┘
                   │  HTTPS
┌──────────────────▼──────────────────────────┐
│  taskchampion-sync-server (selbst betrieben)│
└─────────────────────────────────────────────┘
```

Die Replica liegt unter `~/.local/share/vergissmeinnicht/replica/`. Die App
fasst das Datenverzeichnis der Taskwarrior-CLI **nicht** an — beide sind
unabhängige TaskChampion-Replicas, die über denselben Sync-Server
konvergieren; genau wie die macOS-App und die CLI auf anderen Rechnern.

Design-Begründungen (Speicher-Layout, `u32`-Working-Set-ID, Replica-Lebenszyklus):
[`docs/architecture.md`](docs/architecture.md).

## Download

Release-Tarballs (dynamisch gelinktes x86_64, gebaut auf Arch Linux) liegen
auf der [Releases-Seite](https://github.com/hnsstrk/vergissmeinnicht-kde/releases).
Zur Laufzeit werden Qt 6, Kirigami 6, Kirigami Addons, ki18n und
qqc2-desktop-style benötigt. Außerhalb aktueller Rolling-Release-Distributionen
ist der **Build aus den Quellen der empfohlene Weg** — siehe unten.

## Voraussetzungen

- Qt 6 (qt6-base, qt6-declarative)
- KDE Frameworks 6: Kirigami, Kirigami Addons, ki18n, qqc2-desktop-style,
  Breeze-Icons
- Rust-Toolchain (stable)
- gettext (`msgfmt`, für die Übersetzungskataloge)

Auf Arch und Derivaten:

```sh
pacman -S --needed rust qt6-base qt6-declarative kirigami kirigami-addons \
    ki18n qqc2-desktop-style breeze-icons gettext
```

## Bauen

```sh
# Debug-Build bauen und starten
cargo build
./target/debug/vergissmeinnicht

# Oder bauen + nach ~/.local installieren (Binary, Desktop-Datei, Icon, Übersetzungen)
scripts/install-local.sh
```

Testsuite:

```sh
cargo test --workspace
```

Details (Toolchain, QML-/Bridge-Registrierung, Headless-Testhaken
`--test-flow`/`--test-grab`): [`docs/building.md`](docs/building.md).

## Sync einrichten

1. Eigenen [taskchampion-sync-server](https://github.com/GothenburgBitFactory/taskchampion-sync-server)
   betreiben (oder einen bestehenden nutzen).
2. In der App **Einstellungen → Synchronisation** öffnen und URL, Client-ID
   und Encryption-Secret eintragen. Sie landen im Secret Service
   (unter Plasma: KWallet).
3. **Speichern und Sync testen** klicken. Fertig.

App und `task`-CLI auf anderen Rechnern gleichen sich über den Sync-Server
ab. TaskChampion löst Konflikte CRDT-artig über sein Operation-Log.

## Repo-Aufbau

```
.
├── core/               Rust-Kern: taskchampion-Wrapper (TaskStore, TaskInfo)
│   └── examples/       seed_demo, sync_roundtrip (E2E gegen laufenden Server)
├── app/                Kirigami-App
│   ├── src/            cxx-qt-Bridge, Filter, Parser, State, Backups
│   │   └── ai/         Opt-in-KI-Assistenz (im Aufbau): LLM-Client,
│   │                   Prompt-Bausteine, validierte Entwürfe,
│   │                   Konserven-Mock, Anfrage-Worker, Diktat (Aufnahme +
│   │                   Spracherkennung)
│   ├── qml/            Hauptfenster, Seitenleiste, Dialoge
│   └── cpp/            kleine Shims (KLocalizedContext, Fenster-Grab)
├── data/               Desktop-Datei, Icon, AppStream-Metainfo
├── po/                 gettext-Vorlage + englischer Katalog
├── scripts/            install-local.sh
└── docs/               Architektur, Build, Backup & Restore
```

## Hooks: bewusst nicht im Scope

Taskwarrior-Hooks sind ein Feature der `task`-CLI, nicht der
TaskChampion-Bibliothek. Entsprechende Funktionen (Erinnerungen, Validierung)
sind nativ umgesetzt — dieselbe Entscheidung wie in der macOS-App.

## Danksagungen

- [Taskwarrior](https://taskwarrior.org) und das GothenburgBitFactory-Team für
  [TaskChampion](https://github.com/GothenburgBitFactory/taskchampion) und den
  Sync-Server.
- [KDAB](https://www.kdab.com) für [cxx-qt](https://github.com/KDAB/cxx-qt).
- Die KDE-Community für Kirigami und die Frameworks.

## Lizenz

[MIT](LICENSE).
