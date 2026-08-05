import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import QtQuick.Dialogs as Dialogs

// Kategorie „Wartung" der Einstellungen (UI-4): Datensicherung und Reparatur
// zusammen auf einer Seite, Wortlaut unverändert aus dem alten Dialog — die
// früheren Zwischenüberschriften sind jetzt Karten-Header.
FormCard.FormCardPage {
    id: page

    required property var app

    // Der SettingsDialog (ConfigurationView-Wrapper) — Anmeldung analog
    // SyncSettingsPage, Ziel des Testhakens für die Dialog-Verankerung.
    property var besitzer: null

    title: i18n("Wartung")

    property var backups: []

    function refreshBackups() {
        backups = JSON.parse(app.backupsJson() || "[]")
    }

    Component.onCompleted: {
        refreshBackups()
        if (besitzer)
            besitzer.maintenancePage = page
    }

    // Testhaken (UI-8, #35): öffnet die Lösch-Bestätigung über denselben
    // Pfad wie der Knopf und meldet, ob beide Bestätigungsdialoge im
    // Overlay des Fensters dieser Seite (dem Einstellungsfenster) verankert
    // sind. Beweist die Verankerung, nicht die Pixel — die zeigt der
    // --test-grab-Screenshot mit offenem Dialog.
    function testOeffnePurgeBestaetigung() {
        purgeConfirm.frozenDays = purgeButton.ageDays[purgeAgeCombo.currentIndex]
        purgeConfirm.kandidaten = JSON.parse(app.purgeCandidatesJson(purgeConfirm.frozenDays))
        purgeConfirm.open()
        const overlay = page.QQC2.Overlay.overlay
        return overlay !== null
               && purgeConfirm.parent === overlay
               && restoreConfirm.parent === overlay
    }

    FormCard.FormHeader {
        title: i18n("Wartung — Datensicherung")
    }

    FormCard.FormCard {
        FormCard.FormTextDelegate {
            text: i18n("Automatische Backups")
            description: i18n("Vor jeder Synchronisierung wird ein Backup erstellt; die letzten 10 werden aufbewahrt.")
        }

        FormCard.FormButtonDelegate {
            text: i18n("Backup jetzt erstellen")
            icon.name: "document-save"
            onClicked: {
                page.app.backupNow()
                page.refreshBackups()
            }
        }

        FormCard.FormButtonDelegate {
            text: i18n("Alle Aufgaben exportieren (JSON) …")
            description: i18n("Taskwarrior-Exportformat, inklusive UDAs.")
            icon.name: "document-export"
            onClicked: exportFileDialog.open()
        }

        FormCard.FormButtonDelegate {
            text: i18n("Backup-Ordner öffnen")
            icon.name: "folder-open"
            onClicked: Qt.openUrlExternally("file://" + page.app.backupFolder())
        }

        FormCard.FormComboBoxDelegate {
            id: restoreCombo
            text: i18n("Backup wiederherstellen")
            model: page.backups.map(b => b.filename + " (" + Math.round(b.size_bytes / 1024) + " KiB)")
            onActivated: restoreConfirm.open()
        }

        FormCard.FormTextDelegate {
            visible: page.backups.length === 0
            description: i18n("Noch keine Backups vorhanden.")
        }
    }

    FormCard.FormHeader {
        title: i18n("Wartung — Aufräumen")
    }

    FormCard.FormCard {
        FormCard.FormComboBoxDelegate {
            id: purgeAgeCombo
            text: i18n("Mindestalter erledigter Aufgaben")
            description: i18n("Alter ab letzter Änderung; Wiederholungs-Aufgaben der Taskwarrior-CLI bleiben unberührt.")
            model: [i18n("1 Monat"), i18n("1 Quartal"), i18n("6 Monate"), i18n("1 Jahr")]
            currentIndex: 0
        }

        FormCard.FormButtonDelegate {
            id: purgeButton
            // Tage je Combo-Eintrag — Zählung und Löschung nutzen denselben Wert.
            readonly property var ageDays: [30, 90, 180, 365]
            property int lastResult: -2
            text: i18n("Erledigte Aufgaben löschen …")
            icon.name: "edit-delete"
            description: {
                if (lastResult === -2)
                    return i18n("Zeigt vor dem Löschen die genaue Anzahl; vorher wird automatisch ein Backup angelegt.")
                if (lastResult < 0)
                    return page.app.errorMessage.length > 0
                           ? i18n("Löschen fehlgeschlagen: %1", page.app.errorMessage)
                           : i18n("Löschen fehlgeschlagen.")
                return i18np("1 erledigte Aufgabe gelöscht — mit Strg+Z umkehrbar.",
                             "%1 erledigte Aufgaben gelöscht — mit Strg+Z umkehrbar.", lastResult)
            }
            onClicked: {
                // Kandidatenmenge JETZT einfrieren: Der Dialog bestätigt genau
                // diese UUIDs — Aufgaben, die erst während des offenen Dialogs
                // über die Altersschwelle rutschen, werden nie mitgelöscht.
                purgeConfirm.frozenDays = ageDays[purgeAgeCombo.currentIndex]
                purgeConfirm.kandidaten = JSON.parse(page.app.purgeCandidatesJson(purgeConfirm.frozenDays))
                purgeConfirm.open()
            }
        }
    }

    FormCard.FormHeader {
        title: i18n("Wartung — Reparatur")
    }

    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            id: repairButton
            property int lastResult: -2
            text: i18n("Legacy-Aufgaben reparieren")
            description: {
                if (lastResult === -2)
                    return i18n("Überführt Token-Syntax in Titeln (+tag, project:, due:, priority:) in echte Eigenschaften.")
                if (lastResult < 0)
                    return page.app.errorMessage.length > 0
                           ? i18n("Reparatur fehlgeschlagen: %1", page.app.errorMessage)
                           : i18n("Reparatur fehlgeschlagen.")
                return i18np("1 Aufgabe repariert.", "%1 Aufgaben repariert.", lastResult)
            }
            icon.name: "tools-wizard"
            onClicked: lastResult = page.app.repairLegacyTasks()
        }
    }

    Dialogs.FileDialog {
        id: exportFileDialog
        title: i18n("Aufgaben exportieren")
        fileMode: Dialogs.FileDialog.SaveFile
        nameFilters: [i18n("JSON-Dateien (*.json)")]
        defaultSuffix: "json"
        onAccepted: page.app.exportTasksTo(selectedFile.toString())
    }

    // Bestätigung fürs Aufräumen (UI-5): nennt die exakte Anzahl der beim
    // Öffnen eingefrorenen Kandidatenliste; das Löschen übergibt genau diese
    // UUIDs — nie mehr als bestätigt (Vorbild: restoreConfirm).
    Kirigami.PromptDialog {
        id: purgeConfirm
        // Verankerung im Einstellungsfenster (UI-8, #35): Kirigami.Dialog
        // hängt sich standardmäßig an applicationWindow().overlay — das löst
        // über den Erzeugungskontext der Seite zum HAUPTfenster auf, der
        // Dialog läge unsichtbar hinter dem modalen ConfigWindow. Deshalb
        // ausdrücklich das Overlay des Fensters, in dem die Seite liegt.
        parent: page.QQC2.Overlay.overlay
        // Beim Öffnen eingefrorene Kandidaten-UUIDs samt zugehöriger Schwelle.
        property var kandidaten: []
        property int frozenDays: 0
        readonly property int count: kandidaten.length
        title: i18n("Erledigte Aufgaben löschen")
        subtitle: count > 0
                  ? i18np("1 erledigte Aufgabe ist älter als „%2“ und wird endgültig gelöscht. Vorher wird automatisch ein Backup angelegt; Strg+Z macht die Aktion rückgängig.",
                          "%1 erledigte Aufgaben sind älter als „%2“ und werden endgültig gelöscht. Vorher wird automatisch ein Backup angelegt; Strg+Z macht die Aktion rückgängig.",
                          count, purgeAgeCombo.currentText)
                  : i18n("Keine erledigte Aufgabe ist älter als „%1“ — es gibt nichts zu löschen.", purgeAgeCombo.currentText)
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Löschen")
                icon.name: "edit-delete"
                enabled: purgeConfirm.count > 0
                onTriggered: {
                    purgeButton.lastResult =
                        page.app.purgeCompletedFrozen(purgeConfirm.kandidaten, purgeConfirm.frozenDays)
                    purgeConfirm.close()
                    page.refreshBackups()
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: restoreConfirm
        // Verankerung im Einstellungsfenster — siehe purgeConfirm (UI-8, #35).
        parent: page.QQC2.Overlay.overlay
        title: i18n("Backup wiederherstellen")
        subtitle: page.backups[restoreCombo.currentIndex]
                  ? i18n("Die aktuelle Replica wird durch „%1“ ersetzt. Vorher wird automatisch ein Sicherheits-Backup angelegt.", page.backups[restoreCombo.currentIndex].filename)
                  : ""
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Wiederherstellen")
                icon.name: "edit-undo"
                onTriggered: {
                    const entry = page.backups[restoreCombo.currentIndex]
                    if (entry)
                        page.app.restoreBackupFile(entry.filename)
                    restoreConfirm.close()
                    page.refreshBackups()
                }
            }
        ]
    }
}
