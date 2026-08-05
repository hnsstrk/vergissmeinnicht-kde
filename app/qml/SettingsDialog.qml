import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.settings as KirigamiSettings

// Einstellungen in Kategorien (UI-4, #30): ConfigurationView mit Seitenleiste
// — Allgemein / Synchronisation / KI-Assistent / Wartung. Die Seiten liegen
// in eigenen QML-Dateien (…SettingsPage.qml); `window` und `appContainer`
// setzt Main.qml. CategorizedSettings ist seit Addons 1.3 deprecated und
// wird bewusst nicht verwendet.
KirigamiSettings.ConfigurationView {
    id: dialog

    // Die AppContainer-Instanz für die Seiten: sie entstehen per
    // Qt.createComponent außerhalb des Main-Scopes und sehen die `app`-Id
    // nicht — deshalb Weitergabe als initialProperty.
    property var appContainer

    // Von SyncSettingsPage bei Component.onCompleted gesetzt — Ziel der
    // unveränderten Testhaken-Verträge (testValues/testPoints/saveSync).
    property var syncPage: null

    // Von MaintenanceSettingsPage gesetzt — Ziel der Regressionswache für
    // die Dialog-Verankerung im Einstellungsfenster (UI-8, #35).
    property var maintenancePage: null

    // Von AiSettingsPage gesetzt — Ziel des Popup-Testhakens (UI-7, #34).
    property var aiPage: null

    title: i18n("Einstellungen")

    modules: [
        KirigamiSettings.ConfigurationModule {
            moduleId: "general"
            text: i18n("Allgemein")
            icon.name: "configure"
            page: () => Qt.createComponent("de.hnsstrk.vergissmeinnicht", "GeneralSettingsPage")
            initialProperties: () => ({ app: dialog.appContainer })
        },
        KirigamiSettings.ConfigurationModule {
            moduleId: "sync"
            text: i18n("Synchronisation")
            icon.name: "state-sync"
            page: () => Qt.createComponent("de.hnsstrk.vergissmeinnicht", "SyncSettingsPage")
            initialProperties: () => ({ app: dialog.appContainer, besitzer: dialog })
        },
        KirigamiSettings.ConfigurationModule {
            moduleId: "ai"
            text: i18n("KI-Assistent")
            icon.name: "applications-science"
            page: () => Qt.createComponent("de.hnsstrk.vergissmeinnicht", "AiSettingsPage")
            initialProperties: () => ({ app: dialog.appContainer, besitzer: dialog })
        },
        KirigamiSettings.ConfigurationModule {
            moduleId: "maintenance"
            text: i18n("Wartung")
            icon.name: "tools-wizard"
            page: () => Qt.createComponent("de.hnsstrk.vergissmeinnicht", "MaintenanceSettingsPage")
            initialProperties: () => ({ app: dialog.appContainer, besitzer: dialog })
        }
    ]

    // Bisheriger Einstiegspunkt, jetzt mit optionaler Kategorie-Vorwahl
    // (moduleId) — genutzt vom UI-Test, der direkt auf der Sync-Seite landet.
    function openSettings(moduleId) {
        open(moduleId ?? "")
    }

    // Pendant zum früheren Fenster-close(): schließt das Einstellungsfenster,
    // falls es offen ist (das ConfigWindow zerstört sich dabei selbst).
    function close() {
        if (configViewItem)
            configViewItem.close()
    }

    // ── Testhaken-Delegation (Verträge unverändert, siehe SyncSettingsPage) ──

    function saveSync() {
        if (syncPage)
            syncPage.saveSync()
    }

    function testValues() {
        return syncPage ? syncPage.testValues() : {}
    }

    function testPoints() {
        return syncPage ? syncPage.testPoints() : {}
    }
}
