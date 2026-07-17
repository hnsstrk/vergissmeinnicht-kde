// Seed a TaskChampion replica with a deterministic demo dataset for screenshots
// and manual testing. Usage:
//
//     cargo run --release --example seed_demo -- <replica-path>
//
// The replica directory will be created if it does not exist. Existing tasks
// are not deleted; run against an empty directory for a clean dataset.

use std::env;
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
        project: Some("household"),
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

fn main() {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: cargo run --release --example seed_demo -- <replica-path>");
            process::exit(2);
        }
    };

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
