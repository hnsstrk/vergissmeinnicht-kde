use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
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
