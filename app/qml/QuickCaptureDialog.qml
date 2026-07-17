import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Schnelleingabe (Strg+N): Titel mit Taskwarrior-Token-Syntax (+tag,
// project:, due:, priority:) und Live-Vorschau, plus optionale strukturierte
// Felder. Strukturierte Felder gewinnen gegenüber Tokens.
FormCard.FormCardDialog {
    id: dialog

    title: i18n("Neue Aufgabe")
    standardButtons: QQC2.Dialog.Cancel
    width: Math.min(root.width - Kirigami.Units.gridUnit * 4, Kirigami.Units.gridUnit * 28)

    property var preview: ({})

    function openCapture() {
        titleField.text = ""
        notesArea.text = ""
        projectField.editText = ""
        tagsField.text = ""
        dueCombo.currentIndex = 0
        dueDate.value = new Date()
        priorityCombo.currentIndex = 0
        recurCombo.currentIndex = 0
        preview = {}
        open()
        titleField.forceActiveFocus()
    }

    function updatePreview() {
        preview = JSON.parse(app.quickCapturePreviewJson(titleField.text))
    }

    function commit() {
        updatePreview()
        const p = preview
        const title = (p.description ?? "").trim()
        if (title.length === 0)
            return
        // Strukturierte Felder überschreiben Tokens; Tags werden vereinigt.
        const project = projectField.editText.trim().length > 0
                        ? projectField.editText.trim()
                        : (p.project ?? "")
        const tokenTags = p.tags ?? []
        const fieldTags = tagsField.text.split(/\s+/).filter(t => t.length > 0)
        const tags = Array.from(new Set(tokenTags.concat(fieldTags))).join(" ")

        let due = 0
        const presets = ["", "today", "tomorrow", "+1w", "date"]
        const preset = presets[dueCombo.currentIndex]
        if (preset === "date")
            due = Math.floor(dueDate.value.getTime() / 1000)
        else if (preset !== "")
            due = app.parseDueToken(preset)
        else if (p.due)
            due = app.parseDueToken(p.due)

        const priorities = ["", "H", "M", "L"]
        let priority = priorities[priorityCombo.currentIndex]
        if (priority === "" && p.priority)
            priority = p.priority

        const recurs = ["", "daily", "weekly", "monthly", "yearly"]
        const recur = recurs[recurCombo.currentIndex]

        const ok = app.addTaskDetailed(title, project, tags, due, priority, recur, notesArea.text)
        if (ok)
            dialog.close()
    }

    FormCard.FormTextFieldDelegate {
        id: titleField
        label: i18n("Titel")
        placeholderText: i18n("z. B. Bericht schreiben +arbeit project:Büro due:tomorrow")
        onTextChanged: dialog.updatePreview()
        onAccepted: dialog.commit()
    }

    // Live-Vorschau der erkannten Tokens.
    FormCard.FormTextDelegate {
        visible: {
            const p = dialog.preview
            return (p.tags ?? []).length > 0 || !!p.project || !!p.due || !!p.priority
        }
        text: i18n("Erkannt")
        description: {
            const p = dialog.preview
            const parts = []
            if (p.project) parts.push(i18n("Projekt: %1").arg(p.project))
            for (const t of (p.tags ?? [])) parts.push("#" + t)
            if (p.due) parts.push(p.dueParsed ? i18n("Fällig: %1").arg(p.due) : i18n("Fällig (nicht erkannt): %1").arg(p.due))
            if (p.priority) parts.push(i18n("Priorität: %1").arg(p.priority))
            return parts.join(" · ")
        }
    }

    FormCard.FormTextAreaDelegate {
        id: notesArea
        label: i18n("Notizen")
    }

    FormCard.FormDelegateSeparator {}

    FormCard.FormComboBoxDelegate {
        id: projectField
        text: i18n("Projekt")
        editable: true
        model: [""].concat(root.projects.map(p => p.name))
    }

    FormCard.FormTextFieldDelegate {
        id: tagsField
        label: i18n("Tags")
        placeholderText: i18n("durch Leerzeichen getrennt")
    }

    FormCard.FormComboBoxDelegate {
        id: dueCombo
        text: i18n("Fällig")
        model: [i18n("Keine Angabe"), i18n("Heute"), i18n("Morgen"), i18n("+1 Woche"), i18n("Datum wählen …")]
    }

    FormCard.FormDateTimeDelegate {
        id: dueDate
        visible: dueCombo.currentIndex === 4
        text: i18n("Fällig am")
        dateTimeDisplay: FormCard.FormDateTimeDelegate.DateTimeDisplay.Date
    }

    FormCard.FormComboBoxDelegate {
        id: priorityCombo
        text: i18n("Priorität")
        model: [i18n("Keine"), i18n("Hoch (H)"), i18n("Mittel (M)"), i18n("Niedrig (L)")]
    }

    FormCard.FormComboBoxDelegate {
        id: recurCombo
        text: i18n("Wiederholung")
        model: [i18n("Keine"), i18n("Täglich"), i18n("Wöchentlich"), i18n("Monatlich"), i18n("Jährlich")]
    }

    FormCard.FormDelegateSeparator {}

    FormCard.FormButtonDelegate {
        text: i18n("Hinzufügen")
        icon.name: "list-add"
        enabled: (dialog.preview.description ?? "").length > 0
        onClicked: dialog.commit()
    }
}
