// Qt-freies KI-Modul (Worker, Client, Mock) — von der Bridge genutzt.
mod ai;
mod backup;
mod bridge;
mod config;
mod filters;
mod parsers;
mod secrets;
mod state;
mod urgency;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QQuickStyle, QString, QUrl};
use std::pin::Pin;

/// Wächter für die Testhaken (#38): Jeder `--test-*`-Lauf schreibt durch die
/// echten Speicherpfade — der Sync-Abschnitt des Flows leert zeitweise die
/// Server-URL, der Settings-UI-Test tippt in die Sync-Felder. Läuft so ein
/// Hook gegen die echte Konfiguration oder Replica des Nutzers, frisst er
/// dessen Daten (so ging wiederholt die Sync-Server-URL verloren). Deshalb:
/// Testhaken nur, wenn Konfigurations- UND Datenpfad vom Standardort
/// wegzeigen (Wegwerf-`XDG_CONFIG_HOME`/`XDG_DATA_HOME`).
fn verweigere_testlauf_auf_echten_daten() {
    let testlauf = std::env::args().skip(1).any(|a| a.starts_with("--test-"));
    if !testlauf {
        return;
    }
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let config_echt = dirs::config_dir().is_none_or(|d| d == home.join(".config"));
    let daten_echt = dirs::data_dir().is_none_or(|d| d == home.join(".local").join("share"));
    if config_echt || daten_echt {
        eprintln!(
            "TESTGUARD-FAIL: --test-* verweigert — XDG_CONFIG_HOME und XDG_DATA_HOME müssen \
             auf Wegwerf-Verzeichnisse zeigen, sonst überschreiben die Testhaken die echte \
             Konfiguration und Replica des Nutzers (#38)."
        );
        std::process::exit(2);
    }
}

fn main() {
    verweigere_testlauf_auf_echten_daten();

    // KDE-nativer Look für QtQuick Controls, sofern der User nichts anderes erzwingt.
    if std::env::var("QT_QUICK_CONTROLS_STYLE").is_err() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_organization_name(&QString::from("hnsstrk"));
        app.as_mut().set_organization_domain(&QString::from("hnsstrk.de"));
        app.as_mut().set_application_name(&QString::from("vergissmeinnicht"));
        app.as_mut()
            .set_application_display_name(&QString::from("Vergissmeinnicht"));
        app.as_mut()
            .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
    // Verknüpft das Fenster mit der .desktop-Datei (Icon/Task-Manager unter Wayland).
    QGuiApplication::set_desktop_file_name(&QString::from("de.hnsstrk.vergissmeinnicht"));

    // Sprach-Override aus den Einstellungen (leer = Systemsprache).
    let language = config::Settings::load().language;
    if !language.is_empty() && language != "system" {
        bridge::set_ui_language(&QString::from(language.as_str()));
    }

    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        // ki18n-Kontext VOR dem Laden installieren (Kirigami Addons braucht ihn).
        let qml_engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        bridge::install_klocalized_context(qml_engine);
    }

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/de/hnsstrk/vergissmeinnicht/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
