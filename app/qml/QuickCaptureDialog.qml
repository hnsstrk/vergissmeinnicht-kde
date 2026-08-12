import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Schnelleingabe (Strg+N): Titel mit Taskwarrior-Token-Syntax (+tag,
// project:, due:, priority:) und Live-Vorschau, plus optionale strukturierte
// Felder. Strukturierte Felder gewinnen gegenüber Tokens.
FormWindow {
    id: dialog

    title: i18n("Neue Aufgabe")
    width: Kirigami.Units.gridUnit * 28
    height: Kirigami.Units.gridUnit * 28

    property var preview: ({})

    function openCapture() {
        titleField.text = ""
        notesArea.text = ""
        projectField.editText = ""
        tagsField.text = ""
        dueCombo.currentIndex = 0
        dueDate.value = new Date()
        dueCustomField.text = ""
        priorityCombo.currentIndex = 0
        recurCombo.currentIndex = 0
        recurCustomField.text = ""
        preview = {}
        app.clearAiError()
        openWindow()
        titleField.forceActiveFocus()
    }

    function updatePreview() {
        preview = JSON.parse(app.quickCapturePreviewJson(titleField.text))
    }

    // „Mit KI interpretieren" (AI-B1): schickt den Titel-Freitext ans Modell;
    // die Antwort füllt über applyDraft() nur die Dialogfelder — angelegt
    // wird erst mit dem normalen Hinzufügen-Knopf (Vorschlagen, nie ausführen).
    function interpret() {
        // Kein Start, solange der Diktier-Strang läuft — die Phasen des
        // Flusses „aufnehmen → transkribieren → interpretieren" laufen
        // nacheinander (AI-B2).
        if (!app.aiConfigured || app.aiBusy || app.dictationState !== 0
                || titleField.text.trim().length === 0)
            return
        app.startAiInterpret(titleField.text)
    }

    // Füllfunktion (AI-B1): validierter Entwurf → Dialogfelder. Von außen
    // aufrufbar (auch im --test-flow); der Titel bleibt stehen, wenn die KI
    // keinen liefert — die Eingabe des Users wird nie geleert.
    function applyDraft(draft) {
        if ((draft.title ?? "").trim().length > 0)
            titleField.text = draft.title.trim()
        projectField.editText = draft.project ?? ""
        tagsField.text = (draft.tags ?? []).join(" ")
        setDueValue(draft.due ?? "")
        priorityCombo.currentIndex = Math.max(0, ["", "H", "M", "L"].indexOf(draft.priority ?? ""))
        setRecurValue(draft.recur ?? "")
        notesArea.text = draft.notes ?? ""
        updatePreview()
    }

    // Umkehrung der due-Zuordnung aus commit(): Wert → Preset-Index oder
    // Custom-Pfad. Ein absolutes ISO-Datum geht in den Datumswähler, jeder
    // andere gültige Ausdruck in das Freitextfeld „Benutzerdefiniert".
    function setDueValue(token) {
        token = (token ?? "").trim()
        dueCustomField.text = ""
        const presetIndex = ["", "today", "tomorrow", "+1w"].indexOf(token.toLowerCase())
        if (presetIndex >= 0) {
            dueCombo.currentIndex = presetIndex
        } else if (/^\d{4}-\d{2}-\d{2}$/.test(token)) {
            // Mittag statt Mitternacht: robust gegen Zeitzonen-Kanten.
            dueDate.value = new Date(token + "T12:00:00")
            dueCombo.currentIndex = 4
        } else {
            dueCustomField.text = token
            dueCombo.currentIndex = 5
        }
    }

    // Umkehrung der recur-Zuordnung aus commit(): Preset oder Custom-Feld.
    function setRecurValue(token) {
        token = (token ?? "").trim().toLowerCase()
        recurCustomField.text = ""
        const presetIndex = ["", "daily", "weekly", "monthly", "yearly"].indexOf(token)
        if (presetIndex >= 0) {
            recurCombo.currentIndex = presetIndex
        } else {
            recurCustomField.text = token
            recurCombo.currentIndex = 5
        }
    }

    // Feldwerte für den Headless-Flow (--test-flow), Muster wie im
    // SettingsDialog: interne IDs sind außerhalb der Datei nicht sichtbar.
    function testValues() {
        return {
            title: titleField.text,
            project: projectField.editText,
            tags: tagsField.text,
            dueIndex: dueCombo.currentIndex,
            dueCustom: dueCustomField.text,
            dueDate: dueDate.value,
            priorityIndex: priorityCombo.currentIndex,
            recurIndex: recurCombo.currentIndex,
            recurCustom: recurCustomField.text,
            notes: notesArea.text,
        }
    }

    // Ein KI-Ergebnis füllt die Felder nur, solange der Dialog offen ist —
    // ein spät eintreffender Entwurf greift nicht in einen frisch geöffneten
    // Dialog (openCapture setzt alle Felder zurück, close bricht ab).
    Connections {
        target: app
        function onAiDraftJsonChanged() {
            if (dialog.visible)
                dialog.applyDraft(JSON.parse(app.aiDraftJson || "{}"))
        }
        // Diktier-Fluss (AI-B2): das Transkript landet im Titelfeld und
        // läuft automatisch in die Interpretation weiter — nur solange der
        // Dialog offen ist. Ein leerer Wert (Start eines neuen Diktats oder
        // Fehlschlag) füllt nichts.
        function onDictationTextChanged() {
            if (!dialog.visible || app.dictationText.length === 0)
                return
            titleField.text = app.dictationText
            dialog.interpret()
        }
    }

    // Schließen während einer laufenden Anfrage oder eines Diktats verwirft
    // deren Ergebnis — je Strang über den eigenen Abbruchweg (AI-B2).
    onVisibleChanged: {
        if (!visible) {
            if (app.dictationState !== 0)
                app.cancelDictation()
            if (app.aiBusy)
                app.cancelAiRequest()
        }
    }

    Shortcut {
        sequence: "Ctrl+J"
        enabled: dialog.visible && app.aiConfigured
        onActivated: dialog.interpret()
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
        const presets = ["", "today", "tomorrow", "+1w", "date", "custom"]
        const preset = presets[dueCombo.currentIndex]
        if (preset === "date")
            due = Math.floor(dueDate.value.getTime() / 1000)
        else if (preset === "custom")
            due = app.parseDueToken(dueCustomField.text)
        else if (preset !== "")
            due = app.parseDueToken(preset)
        else if (p.due)
            due = app.parseDueToken(p.due)

        const priorities = ["", "H", "M", "L"]
        let priority = priorities[priorityCombo.currentIndex]
        if (priority === "" && p.priority)
            priority = p.priority

        const recurs = ["", "daily", "weekly", "monthly", "yearly", null]
        let recur = recurs[recurCombo.currentIndex]
        if (recur === null)
            recur = recurCustomField.text.trim()

        const ok = app.addTaskDetailed(title, project, tags, due, priority, recur, notesArea.text)
        if (ok)
            dialog.close()
    }

    buttons: [
        QQC2.Button {
            text: i18n("Hinzufügen")
            icon.name: "list-add"
            enabled: (dialog.preview.description ?? "").length > 0
            onClicked: dialog.commit()
        },
        QQC2.Button {
            text: i18n("Abbrechen")
            icon.name: "dialog-cancel"
            onClicked: dialog.close()
        }
    ]

    // Fehler direkt im Fenster — das Banner der Hauptansicht liegt dahinter.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
        type: Kirigami.MessageType.Error
        text: app.errorMessage
        visible: dialog.visible && app.errorMessage.length > 0
    }

    // KI-Fehler im eigenen Kanal (aiError) — nie im globalen errorMessage.
    Kirigami.InlineMessage {
        Layout.fillWidth: true
        Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
        type: Kirigami.MessageType.Error
        text: app.aiError
        visible: dialog.visible && app.aiError.length > 0
    }

    FormCard.FormTextFieldDelegate {
        Layout.fillWidth: true
        id: titleField
        label: i18n("Titel")
        placeholderText: i18n("z. B. Bericht schreiben +arbeit project:Büro due:tomorrow")
        onTextChanged: dialog.updatePreview()
        onAccepted: dialog.commit()
    }

    // „Mit KI interpretieren" (AI-B1) — nur bei konfigurierter KI sichtbar
    // (Spec §3.2). Bewusst ein echter Knopf statt eines Formular-Delegates:
    // als Listenzeile mit Chevron las sich die Aktion wie ein Navigationsziel.
    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.largeSpacing
        Layout.leftMargin: Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.smallSpacing
        visible: app.aiConfigured
        spacing: Kirigami.Units.smallSpacing

        QQC2.BusyIndicator {
            running: app.aiBusy || app.dictationState === 2
            visible: running
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
        }

        QQC2.Label {
            Layout.fillWidth: true
            // Phasen des Flusses „aufnehmen → transkribieren →
            // interpretieren" — jede spricht für sich (AI-B2).
            text: {
                if (app.dictationState === 1)
                    return i18n("Aufnahme läuft — Klick aufs Mikrofon beendet sie.")
                if (app.dictationState === 2)
                    return i18n("Die Aufnahme wird transkribiert …")
                if (app.aiBusy)
                    return i18n("Die KI liest den Titel …")
                return i18n("Freitext genügt — die KI füllt die Felder.")
            }
            wrapMode: Text.WordWrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
        }

        // Diktat verwerfen (AI-B2): bricht Aufnahme oder Transkription ab,
        // das Ergebnis verfällt. Eine laufende LLM-Anfrage bleibt unberührt.
        QQC2.Button {
            id: dictationCancelButton
            visible: app.dictationState !== 0
            display: QQC2.AbstractButton.IconOnly
            icon.name: "dialog-cancel"
            text: i18n("Diktat verwerfen")
            onClicked: app.cancelDictation()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }

        // Mikrofon (AI-B2): sichtbar nur mit vollständiger Diktier-Kette
        // (die Zeile selbst erscheint nur bei konfigurierter KI). Erster
        // Klick nimmt auf (Knopf pulsiert), zweiter beendet und
        // transkribiert; das Transkript läuft automatisch in die
        // Interpretation weiter.
        QQC2.Button {
            id: dictationButton
            visible: app.dictationAvailable
            // Während Transkription oder laufender Interpretation kein
            // neuer Start — die Phasen laufen nacheinander.
            enabled: app.dictationState === 1
                     || (app.dictationState === 0 && !app.aiBusy)
            display: QQC2.AbstractButton.IconOnly
            icon.name: app.dictationState === 1
                       ? "media-record" : "audio-input-microphone"
            text: app.dictationState === 1
                  ? i18n("Aufnahme beenden und übernehmen")
                  : i18n("Diktieren")
            onClicked: app.dictationState === 1
                       ? app.stopDictation() : app.startDictation()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

            // Pulsieren während der Aufnahme; endet die Aufnahme, springt
            // die Deckkraft zurück auf voll.
            SequentialAnimation on opacity {
                running: app.dictationState === 1
                loops: Animation.Infinite
                onRunningChanged: if (!running) dictationButton.opacity = 1
                NumberAnimation { from: 1.0; to: 0.35; duration: 600; easing.type: Easing.InOutQuad }
                NumberAnimation { from: 0.35; to: 1.0; duration: 600; easing.type: Easing.InOutQuad }
            }
        }

        QQC2.Button {
            id: interpretButton
            text: i18n("Mit KI interpretieren")
            icon.name: "tools-wizard"
            // Hervorgehoben: die Aktion ist der Zweck dieser Zeile.
            highlighted: enabled
            enabled: !app.aiBusy && app.dictationState === 0
                     && titleField.text.trim().length > 0
            onClicked: dialog.interpret()
            QQC2.ToolTip.text: i18n("Strg+J")
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
    }

    // Live-Vorschau der erkannten Tokens.
    FormCard.FormTextDelegate {
        Layout.fillWidth: true
        visible: {
            const p = dialog.preview
            return (p.tags ?? []).length > 0 || !!p.project || !!p.due || !!p.priority
        }
        text: i18n("Erkannt")
        description: {
            const p = dialog.preview
            const parts = []
            if (p.project) parts.push(i18n("Projekt: %1", p.project))
            for (const t of (p.tags ?? [])) parts.push("#" + t)
            if (p.due) parts.push(p.dueParsed ? i18n("Fällig: %1", p.due) : i18n("Fällig (nicht erkannt): %1", p.due))
            if (p.priority) parts.push(i18n("Priorität: %1", p.priority))
            return parts.join(" · ")
        }
    }

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
        label: i18n("Tags")
        placeholderText: i18n("durch Leerzeichen getrennt")
    }

    FormCard.FormComboBoxDelegate {
        Layout.fillWidth: true
        id: dueCombo
        text: i18n("Fällig")
        model: [i18n("Keine Angabe"), i18n("Heute"), i18n("Morgen"), i18n("+1 Woche"), i18n("Datum wählen …"), i18n("Benutzerdefiniert …")]
    }

    FormCard.FormDateTimeDelegate {
        Layout.fillWidth: true
        id: dueDate
        visible: dueCombo.currentIndex === 4
        text: i18n("Fällig am")
        dateTimeDisplay: FormCard.FormDateTimeDelegate.DateTimeDisplay.Date
    }

    // Freitext-Fälligkeit (Taskwarrior-Ausdruck) — Custom-Muster wie beim
    // recur-Feld im DetailDialog; parseDueToken liefert 0 für Unbekanntes.
    FormCard.FormTextFieldDelegate {
        Layout.fillWidth: true
        id: dueCustomField
        visible: dueCombo.currentIndex === 5
        label: i18n("Fälligkeits-Ausdruck")
        placeholderText: i18n("z. B. +3d, eow oder 2026-12-31")
        status: text.trim().length === 0 || app.parseDueToken(text) > 0
                ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
        statusMessage: text.trim().length === 0 || app.parseDueToken(text) > 0
                       ? "" : i18n("Nicht erkannter Ausdruck")
    }

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

    FormCard.FormTextAreaDelegate {
        Layout.fillWidth: true
        id: notesArea
        label: i18n("Notizen")
    }
}
