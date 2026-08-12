// Seed a TaskChampion replica with a deterministic demo dataset for screenshots
// and manual testing. Usage:
//
//     cargo run --release --example seed_demo -- <replica-path> [--ai-config <config-home>]
//
// The replica directory will be created if it does not exist. Existing tasks
// are not deleted; run against an empty directory for a clean dataset.
//
// `--ai-config <config-home>` (AI-B4, #16) schreibt zusätzlich eine minimale
// Demo-Konfiguration nach `<config-home>/vergissmeinnicht/config.json` —
// englische Oberfläche, Ollama-Basis-URL und das im README empfohlene Modell.
// Basis-URL plus Modellname genügen für `aiConfigured`, damit Screenshot-Läufe
// die KI-Bedienelemente zeigen; ein laufender Server oder ein API-Key ist
// nicht nötig. `<config-home>` ist der Wert, der dem App-Start als
// `XDG_CONFIG_HOME` mitgegeben wird. Eine vorhandene config.json wird nie
// überschrieben — Schutz davor, dass jemand versehentlich sein echtes
// `~/.config` angibt.

use std::env;
use std::path::Path;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vergissmeinnicht_core::TaskStore;

struct Demo {
    description: &'static str,
    project: Option<&'static str>,
    tags: &'static [&'static str],
    due_offset_days: Option<i64>,
    priority: Option<&'static str>,
    annotation: Option<&'static str>,
}

const DEMO_TASKS: &[Demo] = &[
    Demo {
        description: "Pay car insurance invoice",
        project: Some("finance"),
        tags: &["urgent"],
        due_offset_days: Some(-2),
        priority: Some("H"),
        annotation: Some("Invoice #4711 is in the inbox."),
    },
    Demo {
        description: "Weekly meal prep",
        project: Some("household"),
        tags: &["routine"],
        due_offset_days: Some(0),
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Review pull request: sync retries",
        project: Some("vergissmeinnicht"),
        tags: &["code", "review"],
        due_offset_days: Some(1),
        priority: Some("M"),
        annotation: None,
    },
    Demo {
        description: "5k run in the park",
        project: Some("health"),
        tags: &["sport"],
        due_offset_days: Some(0),
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Book dentist appointment",
        project: Some("admin"),
        tags: &["phone"],
        due_offset_days: Some(14),
        priority: Some("M"),
        annotation: None,
    },
    Demo {
        description: "Plan weekend trip with Anna",
        project: Some("family"),
        tags: &[],
        due_offset_days: Some(7),
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Read \"Designing Data-Intensive Applications\"",
        project: Some("learning"),
        tags: &["reading"],
        due_offset_days: None,
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Prepare board game night",
        project: Some("leisure"),
        tags: &["friends"],
        due_offset_days: Some(3),
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Declutter the basement",
        // Gepunktetes Subprojekt — zeigt die Projekt-Hierarchie in der Sidebar.
        project: Some("household.basement"),
        tags: &["project"],
        due_offset_days: None,
        priority: Some("L"),
        annotation: None,
    },
    Demo {
        description: "Replace smoke detector batteries",
        project: Some("household"),
        tags: &["maintenance"],
        due_offset_days: Some(10),
        priority: None,
        annotation: None,
    },
    Demo {
        description: "Draft App Store release notes",
        project: Some("vergissmeinnicht"),
        tags: &["release"],
        due_offset_days: Some(21),
        priority: Some("M"),
        annotation: None,
    },
    Demo {
        description: "Call grandma about Sunday lunch",
        project: Some("family"),
        tags: &[],
        due_offset_days: Some(2),
        priority: None,
        annotation: None,
    },
];

// Demo-Konfiguration für Screenshot-Läufe: alle übrigen Felder bekommen beim
// Laden ihre serde-Defaults (`#[serde(default)]` in app/src/config.rs). Das
// Modell ist die gemessene Empfehlung aus dem README; die Sprache ist
// Englisch, weil die Screenshot-DoD englische Locale verlangt (die kompilierte
// en.mo kommt separat dazu, siehe docs/building.md).
const DEMO_AI_CONFIG: &str = r#"{
  "language": "en",
  "ai_base_url": "http://localhost:11434/v1",
  "ai_model": "gemma4:12b"
}
"#;

/// Schreibt die Demo-Konfiguration nach `<config-home>/vergissmeinnicht/
/// config.json`. Eine vorhandene Datei bleibt unangetastet (mit Hinweis) —
/// so kann der Aufruf nie eine echte Konfiguration zerstören.
fn write_ai_config(config_home: &str) -> std::io::Result<()> {
    let dir = Path::new(config_home).join("vergissmeinnicht");
    let datei = dir.join("config.json");
    if datei.exists() {
        println!("ai config untouched (already exists): {}", datei.display());
        return Ok(());
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&datei, DEMO_AI_CONFIG)?;
    println!("wrote demo ai config to {}", datei.display());
    Ok(())
}

fn main() {
    // Argumente: erster positionaler Wert ist der Replica-Pfad, optional
    // gefolgt von `--ai-config <config-home>`.
    let args: Vec<String> = env::args().skip(1).collect();
    let mut path: Option<String> = None;
    let mut ai_config: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--ai-config" {
            match args.get(i + 1) {
                Some(wert) => ai_config = Some(wert.clone()),
                None => {
                    eprintln!("--ai-config needs a path (the XDG_CONFIG_HOME the app starts with)");
                    process::exit(2);
                }
            }
            i += 2;
        } else if path.is_none() {
            path = Some(args[i].clone());
            i += 1;
        } else {
            eprintln!("unknown argument: {}", args[i]);
            process::exit(2);
        }
    }
    let path = match path {
        Some(p) => p,
        None => {
            eprintln!(
                "usage: cargo run --release --example seed_demo -- <replica-path> [--ai-config <config-home>]"
            );
            process::exit(2);
        }
    };

    if let Some(config_home) = &ai_config {
        if let Err(e) = write_ai_config(config_home) {
            eprintln!("failed to write demo ai config under {config_home}: {e}");
            process::exit(1);
        }
    }

    let store = match TaskStore::new(path.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to open replica at {path}: {e:?}");
            process::exit(1);
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let day = Duration::from_secs(60 * 60 * 24).as_secs() as i64;

    let mut created = 0usize;
    for demo in DEMO_TASKS {
        let due = demo.due_offset_days.map(|d| now + d * day);
        let tags: Vec<String> = demo.tags.iter().map(|s| s.to_string()).collect();
        let project = demo.project.map(|s| s.to_string());

        let uuid = match store.add_task_full(demo.description.to_string(), project, tags, due) {
            Ok(u) => u,
            Err(e) => {
                eprintln!("add_task_full failed for {:?}: {e:?}", demo.description);
                continue;
            }
        };

        if let Some(prio) = demo.priority {
            if let Err(e) = store.set_priority(uuid.clone(), Some(prio.to_string())) {
                eprintln!("set_priority failed for {uuid}: {e:?}");
            }
        }
        if let Some(note) = demo.annotation {
            if let Err(e) = store.add_annotation(uuid.clone(), note.to_string()) {
                eprintln!("add_annotation failed for {uuid}: {e:?}");
            }
        }

        created += 1;
    }

    println!("seeded {created} demo tasks at {path}");
}
