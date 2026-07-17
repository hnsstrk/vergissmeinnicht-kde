import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Einstellungen: Allgemein / Synchronisation / Wartung (Backups) —
// Pendant zu den macOS-Settings-Tabs, als ein scrollbarer FormCard-Dialog.
Kirigami.Dialog {
    id: dialog

    title: i18n("Einstellungen")
    standardButtons: Kirigami.Dialog.Close
    preferredWidth: Kirigami.Units.gridUnit * 30
    maximumHeight: Math.round(root.height * 0.9)

    property var backups: []

    function openSettings() {
        clientIdField.text = app.syncClientId()
        secretField.text = app.syncSecret()
        serverUrlField.text = app.syncServerUrl
        refreshBackups()
        open()
    }

    function refreshBackups() {
        backups = JSON.parse(app.backupsJson() || "[]")
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

    // Zielpunkte für den synthetischen UI-Test (--test-settings-ui).
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

    // Scrollender Inhalt — Kirigami.Dialog begrenzt auf Fensterhöhe
    // und zeigt bei Überlänge einen Scrollbalken (FormCardDialog kann das nicht).
    ColumnLayout {
        spacing: 0

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Allgemein")
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: defaultFilterCombo
            text: i18n("Standardansicht beim Start")
            readonly property var keys: ["inbox", "today", "todo", "overdue", "duesoon", "upcoming", "all"]
            model: [i18n("Eingang"), i18n("Heute"), i18n("Zu erledigen"), i18n("Überfällig"), i18n("Bald fällig"), i18n("Geplant"), i18n("Alle")]
            currentIndex: Math.max(0, keys.indexOf(app.defaultFilter))
            onActivated: app.setDefaultFilterSetting(keys[currentIndex])
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: languageCombo
            text: i18n("Sprache")
            description: i18n("Änderung wird nach einem Neustart wirksam. Standard-Dialogknöpfe (OK/Abbrechen) folgen der Systemsprache.")
            readonly property var keys: ["", "de", "en"]
            model: [i18n("Systemsprache"), "Deutsch", "English"]
            currentIndex: Math.max(0, keys.indexOf(app.languageSetting()))
            onActivated: app.setLanguageSetting(keys[currentIndex])
        }

        FormCard.FormSpinBoxDelegate {
            Layout.fillWidth: true
            label: i18n("„Bald fällig“-Fenster (Tage)")
            from: 1
            to: 60
            value: app.dueSoonDays
            onValueChanged: {
                if (value !== app.dueSoonDays)
                    app.setDueSoonDaysSetting(value)
            }
        }

        FormCard.FormSwitchDelegate {
            Layout.fillWidth: true
            text: i18n("Erledigte Aufgaben ausblenden")
            checked: app.hideCompleted
            onToggled: app.setHideCompletedSetting(checked)
        }

        FormCard.FormSwitchDelegate {
            Layout.fillWidth: true
            text: i18n("Benachrichtigung bei überfälligen Aufgaben")
            description: i18n("Zusammenfassung beim Programmstart, wenn überfällige Aufgaben vorliegen.")
            checked: app.notifyOverdue
            onToggled: app.setNotifyOverdueSetting(checked)
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Synchronisation")
        }

        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            text: i18n("TaskChampion-Sync-Server")
            description: i18n("Client-ID und Verschlüsselungs-Secret werden im Passwortspeicher des Systems (KWallet/Secret Service) abgelegt.")
        }

        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: serverUrlField
            label: i18n("Server-URL")
            placeholderText: "https://sync.example.org"
        }

        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: clientIdField
            label: i18n("Client-ID (UUID)")
            placeholderText: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        }

        FormCard.FormPasswordFieldDelegate {
            Layout.fillWidth: true
            id: secretField
            label: i18n("Encryption-Secret")
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: autoSyncCombo
            text: i18n("Automatisch synchronisieren")
            readonly property var keys: ["manual", "m5", "m15", "m60", "immediate"]
            model: [i18n("Manuell"), i18n("Alle 5 Minuten"), i18n("Alle 15 Minuten"), i18n("Alle 60 Minuten"), i18n("Sofort nach Änderungen")]
            currentIndex: Math.max(0, keys.indexOf(app.autoSyncMode))
            onActivated: app.setAutoSyncModeSetting(keys[currentIndex])
        }

        FormCard.FormButtonDelegate {
            Layout.fillWidth: true
            id: saveSyncButton
            text: i18n("Speichern und Sync testen")
            icon.name: "state-sync"
            enabled: !app.isSyncing
            onClicked: dialog.saveSync()
        }

        // Direktes Feedback IM Dialog — das Fehlerbanner der Hauptansicht liegt
        // hinter dem modalen Dialog und wäre unsichtbar.
        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            id: syncStatusLine
            property bool saved: false
            visible: saved || app.isSyncing || app.errorMessage.length > 0 || app.lastSyncAt > 0
            text: i18n("Status")
            description: {
                if (app.errorMessage.length > 0)
                    return app.errorMessage
                if (app.isSyncing)
                    return i18n("Synchronisiere …")
                if (saved && app.lastSyncAt > 0)
                    return i18n("Gespeichert — zuletzt synchronisiert: %1",
                                Qt.formatDateTime(new Date(app.lastSyncAt * 1000), Locale.LongFormat))
                if (saved)
                    return i18n("Gespeichert.")
                if (app.lastSyncAt > 0)
                    return i18n("Zuletzt synchronisiert: %1",
                                Qt.formatDateTime(new Date(app.lastSyncAt * 1000), Locale.LongFormat))
                return ""
            }
            descriptionItem.color: app.errorMessage.length > 0
                                   ? Kirigami.Theme.negativeTextColor
                                   : Kirigami.Theme.positiveTextColor
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Wartung — Datensicherung")
        }

        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            text: i18n("Automatische Backups")
            description: i18n("Vor jeder Synchronisierung wird ein Backup erstellt; die letzten 10 werden aufbewahrt.")
        }

        FormCard.FormButtonDelegate {
            Layout.fillWidth: true
            text: i18n("Backup jetzt erstellen")
            icon.name: "document-save"
            onClicked: {
                app.backupNow()
                dialog.refreshBackups()
            }
        }

        FormCard.FormButtonDelegate {
            Layout.fillWidth: true
            text: i18n("Backup-Ordner öffnen")
            icon.name: "folder-open"
            onClicked: Qt.openUrlExternally("file://" + app.backupFolder())
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: restoreCombo
            text: i18n("Backup wiederherstellen")
            model: dialog.backups.map(b => b.filename + " (" + Math.round(b.size_bytes / 1024) + " KiB)")
            onActivated: restoreConfirm.open()
        }

        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            visible: dialog.backups.length === 0
            description: i18n("Noch keine Backups vorhanden.")
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Wartung — Reparatur")
        }

        FormCard.FormButtonDelegate {
            Layout.fillWidth: true
            id: repairButton
            property int lastResult: -2
            text: i18n("Legacy-Aufgaben reparieren")
            description: {
                if (lastResult === -2)
                    return i18n("Überführt Token-Syntax in Titeln (+tag, project:, due:, priority:) in echte Eigenschaften.")
                if (lastResult < 0)
                    return i18n("Reparatur fehlgeschlagen — Details im Fehlerbanner.")
                return i18np("1 Aufgabe repariert.", "%1 Aufgaben repariert.", lastResult)
            }
            icon.name: "tools-wizard"
            onClicked: lastResult = app.repairLegacyTasks()
        }

        Kirigami.PromptDialog {
            id: restoreConfirm
            title: i18n("Backup wiederherstellen")
            subtitle: dialog.backups[restoreCombo.currentIndex]
                      ? i18n("Die aktuelle Replica wird durch „%1“ ersetzt. Vorher wird automatisch ein Sicherheits-Backup angelegt.", dialog.backups[restoreCombo.currentIndex].filename)
                      : ""
            standardButtons: Kirigami.Dialog.Cancel
            customFooterActions: [
                Kirigami.Action {
                    text: i18n("Wiederherstellen")
                    icon.name: "edit-undo"
                    onTriggered: {
                        const entry = dialog.backups[restoreCombo.currentIndex]
                        if (entry)
                            app.restoreBackupFile(entry.filename)
                        restoreConfirm.close()
                        dialog.refreshBackups()
                    }
                }
            ]
        }
    }
}
