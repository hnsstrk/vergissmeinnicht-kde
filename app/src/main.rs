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

fn main() {
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
