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

    // Nach einem Transkript nennt die Bedienzeile den nächsten Schritt
    // (#49/#50) — bis die Interpretation gestartet oder der Dialog neu
    // geöffnet wird.
    property bool transcriptPending: false

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
        transcriptPending = false
        app.clearAiError()
        openWindow()
        titleField.forceActiveFocus()
    }

    function updatePreview() {
        preview = JSON.parse(app.quickCapturePreviewJson(titleField.text))
    }

    // „Mit KI ausfüllen" (AI-B1): schickt den Titel-Freitext ans Modell;
    // die Antwort füllt über applyDraft() nur die Dialogfelder — angelegt
    // wird erst mit dem normalen Hinzufügen-Knopf (Vorschlagen, nie ausführen).
    function interpret() {
        // Kein Start, solange der Diktier-Strang läuft — die Phasen des
        // Flusses „aufnehmen → transkribieren → interpretieren" laufen
        // nacheinander (AI-B2).
        if (!app.aiConfigured || app.aiBusy || app.dictationState !== 0
                || titleField.text.trim().length === 0)
            return
        transcriptPending = false
        app.startAiInterpret(titleField.text)
    }

    // Füllfunktion (AI-B1): validierter Entwurf → Dialogfelder. Von außen
    // aufrufbar (auch im --test-flow). Leere Entwurfsfelder lassen
    // bestehende Werte stehen (#43) — dieselbe Regel, der der Titel von
    // Anfang an folgte: Ein Modell, das zu einem Feld nichts sagt, hat
    // darüber nichts entschieden. Nur gefüllte Felder ersetzen; die
    // Eingabe des Users wird nie geleert.
    function applyDraft(draft) {
        if ((draft.title ?? "").trim().length > 0)
            titleField.text = draft.title.trim()
        if ((draft.project ?? "").trim().length > 0)
            projectField.editText = draft.project.trim()
        if ((draft.tags ?? []).length > 0)
            tagsField.text = draft.tags.join(" ")
        if ((draft.due ?? "").trim().length > 0)
            setDueValue(draft.due)
        const priorityIndex = ["", "H", "M", "L"].indexOf((draft.priority ?? "").trim())
        if (priorityIndex > 0)
            priorityCombo.currentIndex = priorityIndex
        if ((draft.recur ?? "").trim().length > 0)
            setRecurValue(draft.recur)
        if ((draft.notes ?? "").trim().length > 0)
            notesArea.text = draft.notes
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

    // Sperr- und Sichtbarkeitszustände der KI-Zeile für den Headless-Flow
    // (#41) — gleiches Muster wie testValues(): interne IDs bleiben
    // außerhalb der Datei unsichtbar. QQC2-Tooltips teilen sich eine
    // Engine-weite Instanz (ToolTip.toolTip); über sie weist der Flow
    // nach, dass der Tooltip am gesperrten Mikrofon wirklich erscheint.
    function testUiStates() {
        const geteilterTip = QQC2.ToolTip.toolTip
        return {
            rowVisible: aiRow.visible,
            micVisible: dictationButton.visible,
            micEnabled: dictationButton.enabled,
            micChecked: dictationButton.checked,
            micHovered: dictationHover.hovered,
            interpretEnabled: interpretButton.enabled,
            cancelVisible: phaseCancelButton.visible,
            cancelText: phaseCancelButton.text,
            statusText: statusLabel.text,
            sharedTipVisible: geteilterTip ? geteilterTip.visible : false,
            sharedTipText: geteilterTip ? geteilterTip.text : "",
        }
    }

    // Mitte des Mikrofonknopfs in Fensterkoordinaten — Ziel der
    // synthetischen Mausbewegung im Flow (#41).
    function micCenterInWindow() {
        return dictationButton.mapToItem(null, dictationButton.width / 2,
                                         dictationButton.height / 2)
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
        // Diktier-Fluss (#50): das Diktat schreibt nur den Text ins Feld —
        // die Interpretation läuft ausschließlich über den KI-Knopf bzw.
        // Strg+J. Ein nicht leeres Titelfeld bleibt stehen, das Transkript
        // hängt mit trennendem Leerzeichen an (Diktieren in ein halb
        // getipptes Feld darf nichts verwerfen). Ein leerer Wert (Start
        // eines neuen Diktats oder Fehlschlag) füllt nichts.
        function onDictationTextChanged() {
            if (!dialog.visible || app.dictationText.length === 0)
                return
            const getippt = titleField.text.trim()
            titleField.text = getippt.length > 0
                              ? getippt + " " + app.dictationText
                              : app.dictationText
            dialog.transcriptPending = true
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

    // Bedienzeile (#51): in Leserichtung VOR dem Titelfeld — buchstäblich
    // links neben das Feld geht nicht, FormTextFieldDelegate hat ein festes
    // contentItem ohne Slot für nachgestellte Elemente. Die Zeile ist immer
    // sichtbar (#41): Diktat und KI-Interpretation sind unabhängige
    // Stränge, und was nicht eingerichtet ist, wird gesperrt statt
    // versteckt — den Grund nennt der Tooltip. Reihenfolge: beide
    // Aktionsknöpfe links und ortsfest, dann Spinner und Statustext,
    // Abbrechen ganz rechts — dort verdrängt sein Erscheinen nichts.
    RowLayout {
        id: aiRow
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        Layout.leftMargin: Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing

        // Mikrofon (AI-B2, #41): immer sichtbar; ohne vollständige
        // Diktier-Kette gesperrt statt versteckt. Erster Klick nimmt auf,
        // zweiter beendet und transkribiert; das Transkript landet nur im
        // Titelfeld (#50). Der laufende Zustand wird als gedrückter Knopf
        // gezeigt (checked) — das Icon bleibt das Mikrofon, denn ein Knopf
        // benennt seine Aktion, nicht seinen Zustand (#51).
        // Gesperrte Controls bekommen in Qt Quick keine Hover-Ereignisse —
        // der Tooltip mit dem Sonden-Grund hängt deshalb an einem
        // umschließenden Item mit HoverHandler (Muster: TasksPage-Fußzeile).
        Item {
            // Beide Aktionsknöpfe gleich groß (#52).
            Layout.preferredWidth: Math.max(dictationButton.implicitWidth,
                                            interpretButton.implicitWidth)
            Layout.preferredHeight: Math.max(dictationButton.implicitHeight,
                                             interpretButton.implicitHeight)

            QQC2.Button {
                id: dictationButton
                anchors.fill: parent
                // Während Transkription oder laufender Interpretation kein
                // neuer Start — die Phasen laufen nacheinander.
                enabled: app.dictationAvailable
                         && (app.dictationState === 1
                             || (app.dictationState === 0 && !app.aiBusy))
                display: QQC2.AbstractButton.IconOnly
                icon.name: "audio-input-microphone"
                checkable: true
                checked: app.dictationState === 1
                text: app.dictationState === 1
                      ? i18n("Aufnahme beenden und übernehmen")
                      : i18n("Diktieren")
                onClicked: {
                    // Der Klick hat das Binding an checked überschrieben —
                    // wiederherstellen, damit der Zustand weiter aus der
                    // Bridge kommt und ein fehlgeschlagener Start den Knopf
                    // nicht gedrückt zurücklässt.
                    checked = Qt.binding(() => app.dictationState === 1)
                    app.dictationState === 1
                        ? app.stopDictation() : app.startDictation()
                }
                // Unbeschriftete Knöpfe (#52): der Tooltip trägt die ganze
                // Erklärung.
                QQC2.ToolTip.text: app.dictationState === 1
                                   ? i18n("Aufnahme beenden — das Transkript wird ins Titelfeld übernommen.")
                                   : i18n("Diktieren — die Aufnahme wird transkribiert und landet als Text im Titelfeld.")
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

            HoverHandler {
                id: dictationHover
                enabled: !app.dictationAvailable
            }
            QQC2.ToolTip.text: app.dictationUnavailableReason
            QQC2.ToolTip.visible: dictationHover.hovered && !app.dictationAvailable
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }

        // KI-Knopf (#41, #52): nur Icon, gleich groß wie das Mikrofon — die
        // Erklärung wohnt in Tooltip und Hilfe, nicht auf dem Knopf
        // (Nutzerentscheidung). tools-wizard bleibt: Breeze und Adwaita
        // führen kein Chip-/Hirn-/Schaltkreis-Symbol. Ohne konfigurierte KI
        // gesperrt statt versteckt; der Tooltip nennt den festen Grund
        // (aiConfigured kennt nur Basis-URL und Modell). Gleiches
        // HoverHandler-Muster wie beim Mikrofon, weil der gesperrte Knopf
        // selbst kein Hover meldet.
        Item {
            Layout.preferredWidth: Math.max(dictationButton.implicitWidth,
                                            interpretButton.implicitWidth)
            Layout.preferredHeight: Math.max(dictationButton.implicitHeight,
                                             interpretButton.implicitHeight)

            QQC2.Button {
                id: interpretButton
                anchors.fill: parent
                display: QQC2.AbstractButton.IconOnly
                text: i18n("Mit KI ausfüllen")
                icon.name: "tools-wizard"
                // Hervorgehoben: die Aktion ist der Zweck dieser Zeile.
                highlighted: enabled
                enabled: app.aiConfigured && !app.aiBusy
                         && app.dictationState === 0
                         && titleField.text.trim().length > 0
                onClicked: dialog.interpret()
                QQC2.ToolTip.text: i18n("Mit KI ausfüllen — liest den Titel und füllt die Felder darunter aus (Strg+J).")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            HoverHandler {
                id: interpretHover
                enabled: !app.aiConfigured
            }
            QQC2.ToolTip.text: i18n("KI nicht eingerichtet — in den KI-Einstellungen fehlen Adresse oder Modell.")
            QQC2.ToolTip.visible: interpretHover.hovered && !app.aiConfigured
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }

        QQC2.BusyIndicator {
            running: app.aiBusy || app.dictationState === 2
            visible: running
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
        }

        QQC2.Label {
            id: statusLabel
            Layout.fillWidth: true
            // Phasen des Flusses „aufnehmen → transkribieren →
            // interpretieren" — jede spricht für sich (AI-B2). Nach einem
            // Transkript nennt die Zeile den nächsten Schritt (#50); in
            // Ruhe schweigt sie (#49) — die Knöpfe erklären sich über
            // ihre Tooltips.
            text: {
                if (app.dictationState === 1)
                    return i18n("Aufnahme läuft — Klick aufs Mikrofon beendet sie.")
                if (app.dictationState === 2)
                    return i18n("Die Aufnahme wird transkribiert …")
                if (app.aiBusy)
                    return i18n("Die KI liest den Titel …")
                if (dialog.transcriptPending && app.aiConfigured)
                    return i18n("Transkript übernommen — mit Strg+J ausfüllen lassen.")
                return ""
            }
            wrapMode: Text.WordWrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
        }

        // Abbrechen ganz rechts (#51): bedient beide Stränge mit
        // phasenabhängiger Beschriftung — Aufnahme und Transkription über
        // cancelDictation, die laufende KI-Anfrage über cancelAiRequest.
        // Die Backend-Kanäle sind getrennt; der jeweils andere Strang
        // bleibt unberührt.
        QQC2.Button {
            id: phaseCancelButton
            visible: app.dictationState !== 0 || app.aiBusy
            display: QQC2.AbstractButton.IconOnly
            icon.name: "dialog-cancel"
            text: {
                if (app.dictationState === 1)
                    return i18n("Aufnahme verwerfen")
                if (app.dictationState === 2)
                    return i18n("Transkription abbrechen")
                return i18n("KI-Anfrage abbrechen")
            }
            onClicked: app.dictationState !== 0
                       ? app.cancelDictation() : app.cancelAiRequest()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
    }

    FormCard.FormTextFieldDelegate {
        Layout.fillWidth: true
        id: titleField
        label: i18n("Titel")
        placeholderText: i18n("z. B. Bericht schreiben +arbeit project:Büro due:tomorrow")
        onTextChanged: {
            dialog.updatePreview()
            // Leert der Nutzer den Titel, verfällt der Strg+J-Hinweis —
            // sonst forderte die Zeile zu etwas auf, was der gesperrte
            // Knopf verweigert (#49).
            if (text.trim().length === 0)
                dialog.transcriptPending = false
        }
        onAccepted: dialog.commit()
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
