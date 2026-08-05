import QtQuick
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

    title: i18n("Wartung")

    property var backups: []

    function refreshBackups() {
        backups = JSON.parse(app.backupsJson() || "[]")
    }

    Component.onCompleted: refreshBackups()

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

    Kirigami.PromptDialog {
        id: restoreConfirm
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
