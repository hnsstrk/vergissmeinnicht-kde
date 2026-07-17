import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Detail-Editor: Titel, Projekt, Tags, Fällig, Geplant ab, Warten bis,
// Priorität, Wiederholung, Status, Notizen (Annotations) — Pendant zum
// macOS-Detail-Fenster, als FormCard-Dialog nach KDE-HIG.
Kirigami.Dialog {
    id: dialog

    property string uuid: ""
    property var task: null

    title: i18n("Aufgabe bearbeiten")
    standardButtons: Kirigami.Dialog.Save | QQC2.Dialog.Cancel
    preferredWidth: Kirigami.Units.gridUnit * 30
    maximumHeight: Math.round(root.height * 0.9)

    function openFor(taskUuid) {
        uuid = taskUuid
        task = JSON.parse(app.taskJson(taskUuid))
        if (!task)
            return
        titleField.text = task.description
        projectField.editText = task.project ?? ""
        tagsField.text = (task.tags ?? []).join(" ")
        dueSwitch.checked = task.due !== null
        if (task.due !== null)
            dueDate.value = new Date(task.due * 1000)
        scheduledSwitch.checked = task.scheduled !== null
        if (task.scheduled !== null)
            scheduledDate.value = new Date(task.scheduled * 1000)
        waitSwitch.checked = task.wait !== null
        if (task.wait !== null)
            waitDate.value = new Date(task.wait * 1000)
        priorityCombo.currentIndex = { "": 0, "H": 1, "M": 2, "L": 3 }[task.priority ?? ""] ?? 0
        const recurIdx = { "": 0, "daily": 1, "weekly": 2, "monthly": 3, "yearly": 4 }[task.recur ?? ""]
        recurCombo.currentIndex = recurIdx !== undefined ? recurIdx : 5
        recurCustomField.text = recurIdx === undefined ? (task.recur ?? "") : ""
        annotationField.text = ""
        open()
    }

    onAccepted: {
        const priorities = ["", "H", "M", "L"]
        const recurs = ["", "daily", "weekly", "monthly", "yearly", null]
        let recur = recurs[recurCombo.currentIndex]
        if (recur === null)
            recur = recurCustomField.text.trim()
        const ok = app.saveTaskDetail(
            uuid,
            titleField.text,
            projectField.editText,
            tagsField.text,
            dueSwitch.checked ? Math.floor(dueDate.value.getTime() / 1000) : 0,
            scheduledSwitch.checked ? Math.floor(scheduledDate.value.getTime() / 1000) : 0,
            waitSwitch.checked ? Math.floor(waitDate.value.getTime() / 1000) : 0,
            priorities[priorityCombo.currentIndex],
            recur)
        if (!ok)
            dialog.open() // Fehler oben im Banner; Eingaben nicht verwerfen.
    }

    // Scrollender Inhalt — Kirigami.Dialog begrenzt auf Fensterhöhe
    // und zeigt bei Überlänge einen Scrollbalken (FormCardDialog kann das nicht).
    ColumnLayout {
        spacing: 0

        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: titleField
            label: i18n("Titel")
        }

        FormCard.FormDelegateSeparator { Layout.fillWidth: true }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: projectField
            text: i18n("Projekt")
            editable: true
            model: [""].concat(root.projects.map(p => p.name))
        }

        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: tagsField
            label: i18n("Tags (durch Leerzeichen getrennt)")
            placeholderText: i18n("z. B. arbeit dringend")
        }

        FormCard.FormDelegateSeparator { Layout.fillWidth: true }

        FormCard.FormSwitchDelegate {
            Layout.fillWidth: true
            id: dueSwitch
            text: i18n("Fällig")
        }
        FormCard.FormDateTimeDelegate {
            Layout.fillWidth: true
            id: dueDate
            visible: dueSwitch.checked
            text: i18n("Fällig am")
        }

        FormCard.FormSwitchDelegate {
            Layout.fillWidth: true
            id: scheduledSwitch
            text: i18n("Geplant ab")
        }
        FormCard.FormDateTimeDelegate {
            Layout.fillWidth: true
            id: scheduledDate
            visible: scheduledSwitch.checked
            text: i18n("Geplant ab")
        }

        FormCard.FormSwitchDelegate {
            Layout.fillWidth: true
            id: waitSwitch
            text: i18n("Zurückgestellt (warten bis)")
        }
        FormCard.FormDateTimeDelegate {
            Layout.fillWidth: true
            id: waitDate
            visible: waitSwitch.checked
            text: i18n("Warten bis")
        }

        FormCard.FormDelegateSeparator { Layout.fillWidth: true }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: priorityCombo
            text: i18n("Priorität")
            model: [i18n("Keine"), i18n("Hoch (H)"), i18n("Mittel (M)"), i18n("Niedrig (L)")]
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: recurCombo
            text: i18n("Wiederholung")
            model: [i18n("Keine"), i18n("Täglich"), i18n("Wöchentlich"), i18n("Monatlich"), i18n("Jährlich"), i18n("Benutzerdefiniert …")]
        }
        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: recurCustomField
            visible: recurCombo.currentIndex === 5
            label: i18n("Intervall (Nd / Nw / Nm / Ny)")
            placeholderText: i18n("z. B. 3d oder 2w")
            status: app.isValidRecurToken(text) ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
            statusMessage: app.isValidRecurToken(text) ? "" : i18n("Nicht erkanntes Intervall")
        }

        FormCard.FormDelegateSeparator { Layout.fillWidth: true }

        // Status + Reaktivieren + Blockiert-Info
        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            text: i18n("Status")
            description: {
                if (!dialog.task) return ""
                switch (dialog.task.status) {
                case "completed": return i18n("Erledigt")
                case "deleted": return i18n("Gelöscht")
                case "recurring": return i18n("Wiederkehrend (Vorlage)")
                default: return i18n("Ausstehend")
                }
            }
            trailing: QQC2.Button {
                visible: dialog.task && dialog.task.status === "completed"
                text: i18n("Reaktivieren")
                icon.name: "edit-undo"
                onClicked: {
                    app.reactivateTask(dialog.uuid)
                    dialog.close()
                }
            }
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Abhängigkeiten")
        }

        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            visible: dialog.task && dialog.task.isBlocking
            description: i18n("Diese Aufgabe blockiert andere Aufgaben.")
        }

        // Bestehende Abhängigkeiten (hängt ab von …) mit Titel-Auflösung.
        Repeater {
            model: dialog.task ? dialog.task.depends : []
            FormCard.FormTextDelegate {
                Layout.fillWidth: true
                required property var modelData
                readonly property var depTask: JSON.parse(app.taskJson(modelData))
                text: depTask
                      ? i18n("Hängt ab von: %1", depTask.description)
                      : i18n("Hängt ab von unbekannter Aufgabe (%1)", modelData.substring(0, 8))
                description: depTask && depTask.status !== "pending" ? i18n("Erledigt") : ""
                trailing: QQC2.ToolButton {
                    icon.name: "edit-delete-remove"
                    QQC2.ToolTip.text: i18n("Abhängigkeit entfernen")
                    QQC2.ToolTip.visible: hovered
                    onClicked: {
                        app.removeTaskDependency(dialog.uuid, modelData)
                        dialog.task = JSON.parse(app.taskJson(dialog.uuid))
                    }
                }
            }
        }

        FormCard.FormComboBoxDelegate {
            Layout.fillWidth: true
            id: addDependencyCombo
            text: i18n("Abhängigkeit hinzufügen")
            displayText: i18n("Aufgabe wählen …")
            readonly property var candidates: {
                if (!dialog.task) return []
                const existing = dialog.task.depends ?? []
                return JSON.parse(app.pendingTasksJson())
                    .filter(t => t.uuid !== dialog.uuid && existing.indexOf(t.uuid) === -1)
            }
            model: candidates.map(t => (t.wsId !== null ? "#" + t.wsId + " " : "") + t.title)
            onActivated: index => {
                const target = candidates[index]
                if (target) {
                    app.addTaskDependency(dialog.uuid, target.uuid)
                    dialog.task = JSON.parse(app.taskJson(dialog.uuid))
                }
            }
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: Kirigami.Units.smallSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            level: 4
            text: i18n("Notizen")
        }

        Repeater {
            model: dialog.task ? dialog.task.annotations : []
            FormCard.FormTextDelegate {
                Layout.fillWidth: true
                required property var modelData
                text: modelData.description
                description: Qt.formatDateTime(new Date(modelData.entry * 1000), Locale.ShortFormat)
                trailing: QQC2.ToolButton {
                    icon.name: "edit-delete-remove"
                    QQC2.ToolTip.text: i18n("Notiz entfernen")
                    QQC2.ToolTip.visible: hovered
                    onClicked: {
                        app.removeTaskAnnotation(dialog.uuid, modelData.entry)
                        dialog.task = JSON.parse(app.taskJson(dialog.uuid))
                    }
                }
            }
        }

        FormCard.FormTextFieldDelegate {
            Layout.fillWidth: true
            id: annotationField
            label: i18n("Neue Notiz")
            trailing: QQC2.ToolButton {
                icon.name: "list-add"
                enabled: annotationField.text.trim().length > 0
                onClicked: {
                    app.addTaskAnnotation(dialog.uuid, annotationField.text.trim())
                    annotationField.text = ""
                    dialog.task = JSON.parse(app.taskJson(dialog.uuid))
                }
            }
        }

        FormCard.FormTextDelegate {
            Layout.fillWidth: true
            text: i18n("Angelegt")
            visible: dialog.task && dialog.task.entry
            description: dialog.task && dialog.task.entry
                         ? Qt.formatDateTime(new Date(dialog.task.entry * 1000), Locale.LongFormat)
                         : ""
        }
    }
}
