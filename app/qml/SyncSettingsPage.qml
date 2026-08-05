import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Kategorie „Synchronisation" der Einstellungen (UI-4): Felder, Wortlaut und
// „Speichern und Sync testen"-Verhalten unverändert aus dem alten Dialog.
// Die Seite meldet sich beim Besitzer (SettingsDialog.qml) an, damit dessen
// Testhaken-Delegation (testValues/testPoints/saveSync) sie erreicht.
FormCard.FormCardPage {
    id: page

    required property var app
    // Der SettingsDialog (ConfigurationView-Wrapper) — Ziel der Anmeldung.
    required property var besitzer

    title: i18n("Synchronisation")

    Component.onCompleted: {
        clientIdField.text = app.syncClientId()
        secretField.text = app.syncSecret()
        serverUrlField.text = app.syncServerUrl
        besitzer.syncPage = page
    }

    Component.onDestruction: {
        if (besitzer.syncPage === page)
            besitzer.syncPage = null
    }

    // Speichern-Sequenz des Sync-Bereichs — vom Button und vom UI-Test genutzt.
    function saveSync() {
        app.clearError()
        syncStatusLine.saved = false
        app.setSyncServerUrlSetting(serverUrlField.text)
        if (app.setSyncCredentials(clientIdField.text, secretField.text)) {
            syncStatusLine.saved = true
            app.startSync()
        }
    }

    // Aktuelle Feldwerte für den synthetischen UI-Test (--test-settings-ui).
    function testValues() {
        return { url: serverUrlField.text, clientId: clientIdField.text, secret: secretField.text }
    }

    // Zielpunkte für den synthetischen UI-Test (--test-settings-ui) —
    // Fensterkoordinaten des Einstellungsfensters (ConfigWindow).
    function testPoints() {
        function center(item) {
            return item.mapToItem(null, item.width / 2, item.height / 2)
        }
        return {
            url: center(serverUrlField),
            clientId: center(clientIdField),
            secret: center(secretField),
            save: center(saveSyncButton)
        }
    }

    FormCard.FormCard {
        Layout.topMargin: Kirigami.Units.largeSpacing

        FormCard.FormTextDelegate {
            text: i18n("TaskChampion-Sync-Server")
            description: i18n("Client-ID und Verschlüsselungs-Secret werden im Passwortspeicher des Systems (KWallet/Secret Service) abgelegt.")
        }

        FormCard.FormTextFieldDelegate {
            id: serverUrlField
            label: i18n("Server-URL")
            placeholderText: "https://sync.example.org"
        }

        FormCard.FormTextFieldDelegate {
            id: clientIdField
            label: i18n("Client-ID (UUID)")
            placeholderText: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        }

        FormCard.FormPasswordFieldDelegate {
            id: secretField
            label: i18n("Encryption-Secret")
        }

        FormCard.FormComboBoxDelegate {
            id: autoSyncCombo
            text: i18n("Automatisch synchronisieren")
            readonly property var keys: ["manual", "m5", "m15", "m60", "immediate"]
            model: [i18n("Manuell"), i18n("Alle 5 Minuten"), i18n("Alle 15 Minuten"), i18n("Alle 60 Minuten"), i18n("Sofort nach Änderungen")]
            currentIndex: Math.max(0, keys.indexOf(page.app.autoSyncMode))
            onActivated: page.app.setAutoSyncModeSetting(keys[currentIndex])
        }

        FormCard.FormButtonDelegate {
            id: saveSyncButton
            text: i18n("Speichern und Sync testen")
            icon.name: "state-sync"
            enabled: !page.app.isSyncing
            onClicked: page.saveSync()
        }

        // Direktes Feedback IM Dialog — das Fehlerbanner der Hauptansicht liegt
        // hinter dem Einstellungsfenster und wäre unsichtbar.
        FormCard.FormTextDelegate {
            id: syncStatusLine
            property bool saved: false
            visible: saved || page.app.isSyncing || page.app.errorMessage.length > 0 || page.app.lastSyncAt > 0
            text: i18n("Status")
            description: {
                if (page.app.errorMessage.length > 0)
                    return page.app.errorMessage
                if (page.app.isSyncing)
                    return i18n("Synchronisiere …")
                if (saved && page.app.lastSyncAt > 0)
                    return i18n("Gespeichert — zuletzt synchronisiert: %1",
                                Qt.formatDateTime(new Date(page.app.lastSyncAt * 1000), Locale.LongFormat))
                if (saved)
                    return i18n("Gespeichert.")
                if (page.app.lastSyncAt > 0)
                    return i18n("Zuletzt synchronisiert: %1",
                                Qt.formatDateTime(new Date(page.app.lastSyncAt * 1000), Locale.LongFormat))
                return ""
            }
            descriptionItem.color: page.app.errorMessage.length > 0
                                   ? Kirigami.Theme.negativeTextColor
                                   : Kirigami.Theme.positiveTextColor
        }
    }
}
