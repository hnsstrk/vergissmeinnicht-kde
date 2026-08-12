use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::process::Command;

/// Build-Kennung (#54): Kurz-Commit und Commit-Datum, z. B. "54d2572, 2026-08-12".
/// Ohne Git-Verzeichnis (Tarball-Bau) bleibt sie leer — der Über-Dialog zeigt
/// dann nur die Version statt einer leeren Klammer.
///
/// Zielkonflikt Cache vs. Aktualität: build.rs soll nicht bei jedem Bau neu
/// laufen, die Kennung aber auch nicht veralten. Auflösung: Als Datum dient das
/// Commit-Datum statt der Uhr — die Kennung ist damit eine reine Funktion des
/// Git-Stands — und neu gebaut wird genau dann, wenn sich dieser Stand ändert:
/// `HEAD` (Branch-Wechsel, Detach) und `logs/HEAD` (Reflog; wächst bei jedem
/// Commit, Amend, Rebase, Reset) sind als rerun-Anker registriert.
/// Unkommittierte Änderungen ändern die Kennung bewusst nicht.
fn build_kennung() -> String {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
    };

    // rerun-Anker zuerst: `--git-path` löst auch Worktrees korrekt auf.
    for anker in ["HEAD", "logs/HEAD"] {
        if let Some(pfad) = git(&["rev-parse", "--git-path", anker]) {
            if std::path::Path::new(&pfad).exists() {
                println!("cargo:rerun-if-changed={pfad}");
            }
        }
    }

    match (
        git(&["rev-parse", "--short=7", "HEAD"]),
        git(&["show", "-s", "--format=%cs", "HEAD"]),
    ) {
        (Some(commit), Some(datum)) => format!("{commit}, {datum}"),
        _ => String::new(),
    }
}

fn main() {
    // Kompilierzeit-Konstante für Über-Dialog und --version (#54).
    println!("cargo:rustc-env=VM_BUILD_INFO={}", build_kennung());

    // Jede neue QML-Datei und jede neue Bridge-Rust-Datei muss hier registriert
    // werden — Pendant zur xcodeproj-Pflege der macOS-Version.
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("de.hnsstrk.vergissmeinnicht").qml_files([
            "qml/Main.qml",
            "qml/Sidebar.qml",
            "qml/TasksPage.qml",
            "qml/TaskDelegate.qml",
            "qml/FormWindow.qml",
            "qml/DetailDialog.qml",
            "qml/QuickCaptureDialog.qml",
            "qml/SettingsDialog.qml",
            "qml/GeneralSettingsPage.qml",
            "qml/SyncSettingsPage.qml",
            "qml/DictationSettingsPage.qml",
            "qml/AiSettingsPage.qml",
            "qml/MaintenanceSettingsPage.qml",
            "qml/VmComboBoxDelegate.qml",
            "qml/HelpDialog.qml",
            "qml/AboutDialog.qml",
        ]),
    )
    .files(["src/bridge.rs"]);

    // SAFETY: kompiliert nur unser eigenes, eingechecktes C++ (klocalized.cpp).
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.file("cpp/klocalized.cpp");
            cc.file("cpp/grabwindow.cpp");
            cc.file("cpp/inputsim.cpp");
            cc.include("cpp");
            // KF6-Header (ki18n) — die Qt-Basis-Include-Pfade setzt cxx-qt-build
            // selbst, QtQuick (QQuickWindow für den Grab-Shim) aber nicht.
            cc.include("/usr/include/KF6");
            cc.include("/usr/include/KF6/KI18n");
            cc.include("/usr/include/qt6/QtQuick");
            cc.include("/usr/include/qt6/QtQmlIntegration");
        })
    };

    builder.build();

    println!("cargo:rustc-link-lib=KF6I18n");
    println!("cargo:rustc-link-lib=KF6I18nQml");
    println!("cargo:rerun-if-changed=cpp/klocalized.cpp");
    println!("cargo:rerun-if-changed=cpp/klocalized.h");
}
