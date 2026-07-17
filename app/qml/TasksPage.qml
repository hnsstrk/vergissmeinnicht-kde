import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Hauptbereich: Suchfeld, Fehlerbanner, Task-Liste mit Mehrfachauswahl,
// Kontextmenü mit Bulk-Aktionen, Empty-States pro Filter, Sync-Fußzeile.
Kirigami.ScrollablePage {
    id: page

    title: root.filterTitle(app.filterKey)

    // ── Auswahl-Zustand ─────────────────────────────────────────────────────
    property var selection: []
    property int selectionAnchor: -1

    function isSelected(uuid) {
        return selection.indexOf(uuid) !== -1
    }

    function selectSingle(uuid, index) {
        selection = [uuid]
        selectionAnchor = index
    }

    function toggleSelection(uuid, index) {
        const s = selection.slice()
        const i = s.indexOf(uuid)
        if (i === -1)
            s.push(uuid)
        else
            s.splice(i, 1)
        selection = s
        selectionAnchor = index
    }

    function selectRange(index) {
        if (selectionAnchor < 0) {
            selectionAnchor = index
        }
        const uuids = app.visibleUuids(selectionAnchor, index)
        selection = Array.from(uuids)
    }

    function clearSelection() {
        selection = []
        selectionAnchor = -1
    }

    // Aufgeräumte Auswahl nach jedem Modell-Reset: verschwundene UUIDs entfernen.
    Connections {
        target: app
        function onModelReset() {
            const still = Array.from(app.visibleUuids(0, taskList.count - 1))
            page.selection = page.selection.filter(u => still.indexOf(u) !== -1)
        }
    }

    // Kontextmenü-Ziel: die Auswahl, oder die Zeile unter dem Mauszeiger.
    function effectiveTargets(uuid) {
        if (uuid && !isSelected(uuid))
            return [uuid]
        return selection
    }

    // ── Kopfzeile: Suche + Fehlerbanner ─────────────────────────────────────
    header: ColumnLayout {
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            Kirigami.SearchField {
                id: searchField
                Layout.fillWidth: true
                placeholderText: i18n("Suchen — auch projekt:, tag:, status: …")
                text: app.searchQuery
                onAccepted: app.applySearch(text)
                onTextChanged: {
                    if (text !== app.searchQuery)
                        searchDebounce.restart()
                }
                Timer {
                    id: searchDebounce
                    interval: 250
                    onTriggered: app.applySearch(searchField.text)
                }
            }

            QQC2.ToolButton {
                icon.name: "bookmark-new"
                visible: app.searchQuery.trim().length > 0
                text: i18n("Suche speichern")
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                onClicked: saveSearchPrompt.openPrompt()
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
            type: Kirigami.MessageType.Error
            text: app.errorMessage
            visible: app.errorMessage.length > 0
            showCloseButton: true
            onVisibleChanged: if (visible) errorDismiss.restart()
            Timer {
                id: errorDismiss
                interval: 6000
                onTriggered: app.clearError()
            }
            actions: [
                Kirigami.Action {
                    text: i18n("Erneut laden")
                    icon.name: "view-refresh"
                    onTriggered: app.refresh()
                }
            ]
        }
    }

    // ── Fußzeile: Sync-Status ───────────────────────────────────────────────
    footer: RowLayout {
        visible: app.syncConfigured || app.isSyncing
        spacing: Kirigami.Units.smallSpacing

        Item { Layout.fillWidth: true }

        QQC2.BusyIndicator {
            visible: app.isSyncing
            running: app.isSyncing
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
        }

        Kirigami.Icon {
            visible: !app.isSyncing && app.hasLocalChanges
            source: "vcs-locally-modified-unstaged"
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            QQC2.ToolTip.text: i18n("Lokale Änderungen noch nicht synchronisiert")
            QQC2.ToolTip.visible: hoverHandler.hovered
            HoverHandler { id: hoverHandler }
        }

        QQC2.Label {
            text: app.isSyncing
                  ? i18n("Synchronisiere …")
                  : (app.lastSyncAt > 0
                     ? i18n("Zuletzt synchronisiert: %1", Qt.formatDateTime(new Date(app.lastSyncAt * 1000), "hh:mm"))
                     : "")
            opacity: 0.7
            rightPadding: Kirigami.Units.largeSpacing
            bottomPadding: Kirigami.Units.smallSpacing
        }
    }

    // ── Toolbar-Aktionen ────────────────────────────────────────────────────
    actions: [
        Kirigami.Action {
            text: i18n("Neue Aufgabe")
            icon.name: "list-add"
            shortcut: StandardKey.New
            onTriggered: quickCaptureDialog.openCapture()
        },
        Kirigami.Action {
            text: i18n("Erledigt")
            icon.name: "checkbox"
            shortcut: "Ctrl+D"
            enabled: page.selection.length > 0
            onTriggered: app.markDoneBulk(page.selection)
        },
        Kirigami.Action {
            text: i18n("Löschen")
            icon.name: "edit-delete"
            shortcut: "Del"
            enabled: page.selection.length > 0
            onTriggered: deleteConfirm.open()
        },
        Kirigami.Action {
            text: i18n("Sortierung")
            icon.name: "view-sort"
            Kirigami.Action {
                text: i18n("ID")
                checkable: true
                checked: app.sortKey === "id"
                onTriggered: app.setSort("id", app.sortAscending)
            }
            Kirigami.Action {
                text: i18n("Name")
                checkable: true
                checked: app.sortKey === "description"
                onTriggered: app.setSort("description", app.sortAscending)
            }
            Kirigami.Action {
                text: i18n("Angelegt")
                checkable: true
                checked: app.sortKey === "entry"
                onTriggered: app.setSort("entry", app.sortAscending)
            }
            Kirigami.Action {
                text: i18n("Fälligkeit")
                checkable: true
                checked: app.sortKey === "due"
                onTriggered: app.setSort("due", app.sortAscending)
            }
            Kirigami.Action {
                text: i18n("Projekt")
                checkable: true
                checked: app.sortKey === "project"
                onTriggered: app.setSort("project", app.sortAscending)
            }
            Kirigami.Action { separator: true }
            Kirigami.Action {
                text: i18n("Aufsteigend")
                checkable: true
                checked: app.sortAscending
                onTriggered: app.setSort(app.sortKey, true)
            }
            Kirigami.Action {
                text: i18n("Absteigend")
                checkable: true
                checked: !app.sortAscending
                onTriggered: app.setSort(app.sortKey, false)
            }
        },
        Kirigami.Action {
            text: app.isSyncing ? i18n("Synchronisiere …") : i18n("Synchronisieren")
            icon.name: "state-sync"
            enabled: !app.isSyncing
            shortcut: "Ctrl+Shift+S"
            onTriggered: app.startSync()
        },
        Kirigami.Action {
            text: i18n("Erledigte ausblenden")
            icon.name: "view-visible"
            checkable: true
            checked: app.hideCompleted
            shortcut: "Ctrl+Shift+H"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: app.setHideCompletedSetting(!app.hideCompleted)
        },
        Kirigami.Action {
            text: i18n("Berichte")
            displayHint: Kirigami.DisplayHint.AlwaysHide
            Kirigami.Action {
                text: i18n("Blockiert")
                onTriggered: app.applyFilter("blocked")
            }
            Kirigami.Action {
                text: i18n("Blockierend")
                onTriggered: app.applyFilter("blocking")
            }
            Kirigami.Action {
                text: i18n("Nicht blockiert")
                onTriggered: app.applyFilter("unblocked")
            }
        },
        Kirigami.Action {
            text: i18n("Aktualisieren")
            icon.name: "view-refresh"
            shortcut: StandardKey.Refresh
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: app.refresh()
        },
        Kirigami.Action {
            text: i18n("Einstellungen …")
            icon.name: "configure"
            shortcut: "Ctrl+Shift+,"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: settingsDialog.openSettings()
        },
        Kirigami.Action {
            text: i18n("Hilfe (Tastenkürzel und Suche)")
            icon.name: "help-contents"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: helpDialog.open()
        },
        Kirigami.Action {
            text: i18n("Über Vergissmeinnicht")
            icon.name: "help-about"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: aboutDialog.open()
        }
    ]

    // ── Liste ───────────────────────────────────────────────────────────────
    ListView {
        id: taskList
        model: app
        currentIndex: -1
        reuseItems: false
        delegate: TaskDelegate {}

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: taskList.count === 0
            icon.name: {
                if (app.initError.length > 0) return "data-error"
                if (app.searchQuery.length > 0) return "edit-find"
                switch (app.filterKey) {
                case "todo": return "checkbox"
                case "overdue": return "checkmark"
                default: return "mail-folder-inbox"
                }
            }
            text: {
                if (app.initError.length > 0) return i18n("Replica konnte nicht geöffnet werden")
                if (app.searchQuery.length > 0) return i18n("Keine Treffer")
                switch (app.filterKey) {
                case "inbox": return i18n("Der Eingang ist leer")
                case "today": return i18n("Heute ist nichts fällig")
                case "todo": return i18n("Alles erledigt!")
                case "overdue": return i18n("Nichts überfällig")
                case "duesoon": return i18n("Nichts bald fällig")
                case "upcoming": return i18n("Nichts geplant")
                case "waiting": return i18n("Keine wartenden Aufgaben")
                default: return i18n("Keine Aufgaben")
                }
            }
            explanation: {
                if (app.initError.length > 0) return app.initError
                if (app.searchQuery.length > 0) return i18n("Suchbegriffe anpassen oder Operatoren wie projekt:, tag:, status: verwenden.")
                if (app.filterKey === "inbox") return i18n("Alle offenen Aufgaben haben ein Projekt oder Tags.")
                return ""
            }
            helpfulAction: app.initError.length === 0 ? newTaskAction : null
        }
    }

    Kirigami.Action {
        id: newTaskAction
        text: i18n("Neue Aufgabe …")
        icon.name: "list-add"
        onTriggered: quickCaptureDialog.openCapture()
    }

    // ── Kontextmenü (Selektion-fähig) ───────────────────────────────────────
    QQC2.Menu {
        id: contextMenu
        property var targets: []
        property string singleUuid: targets.length === 1 ? targets[0] : ""
        property bool anyCompleted: false

        function popupFor(uuid, completed) {
            targets = page.effectiveTargets(uuid)
            anyCompleted = completed
            popup()
        }

        QQC2.MenuItem {
            text: i18n("Details öffnen")
            icon.name: "document-edit"
            enabled: contextMenu.singleUuid !== ""
            onTriggered: root.openDetail(contextMenu.singleUuid)
        }
        QQC2.MenuItem {
            text: i18n("Erledigt")
            icon.name: "checkbox"
            onTriggered: app.markDoneBulk(contextMenu.targets)
        }
        QQC2.MenuItem {
            text: i18n("Reaktivieren")
            icon.name: "edit-undo"
            visible: contextMenu.anyCompleted && contextMenu.singleUuid !== ""
            onTriggered: app.reactivateTask(contextMenu.singleUuid)
        }
        QQC2.Menu {
            title: i18n("Verschieben auf …")
            icon.name: "clock"
            QQC2.MenuItem {
                text: i18n("Morgen")
                onTriggered: app.bulkSnooze(contextMenu.targets, Math.floor(Date.now() / 1000) + 86400)
            }
            QQC2.MenuItem {
                text: i18n("+3 Tage")
                onTriggered: app.bulkSnooze(contextMenu.targets, Math.floor(Date.now() / 1000) + 3 * 86400)
            }
            QQC2.MenuItem {
                text: i18n("+1 Woche")
                onTriggered: app.bulkSnooze(contextMenu.targets, Math.floor(Date.now() / 1000) + 7 * 86400)
            }
            QQC2.MenuSeparator {}
            QQC2.MenuItem {
                text: i18n("Zurückstellen aufheben")
                onTriggered: app.bulkSnooze(contextMenu.targets, 0)
            }
        }
        QQC2.Menu {
            title: i18n("Projekt setzen")
            icon.name: "folder"
            Repeater {
                model: root.projects
                QQC2.MenuItem {
                    required property var modelData
                    text: modelData.name
                    onTriggered: app.bulkSetProject(contextMenu.targets, modelData.name)
                }
            }
            QQC2.MenuSeparator { visible: root.projects.length > 0 }
            QQC2.MenuItem {
                text: i18n("Neues Projekt …")
                onTriggered: newProjectPrompt.openFor(contextMenu.targets)
            }
            QQC2.MenuItem {
                text: i18n("Kein Projekt")
                onTriggered: app.bulkSetProject(contextMenu.targets, "")
            }
        }
        QQC2.Menu {
            title: i18n("Tag hinzufügen")
            icon.name: "tag"
            Repeater {
                model: root.tagList
                QQC2.MenuItem {
                    required property var modelData
                    text: modelData.name
                    onTriggered: app.bulkAddTag(contextMenu.targets, modelData.name)
                }
            }
            QQC2.MenuSeparator { visible: root.tagList.length > 0 }
            QQC2.MenuItem {
                text: i18n("Neuer Tag …")
                onTriggered: newTagPrompt.openFor(contextMenu.targets)
            }
        }
        QQC2.Menu {
            title: i18n("Priorität")
            icon.name: "emblem-important"
            QQC2.MenuItem { text: i18n("Hoch");    onTriggered: app.bulkSetPriority(contextMenu.targets, "H") }
            QQC2.MenuItem { text: i18n("Mittel");  onTriggered: app.bulkSetPriority(contextMenu.targets, "M") }
            QQC2.MenuItem { text: i18n("Niedrig"); onTriggered: app.bulkSetPriority(contextMenu.targets, "L") }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { text: i18n("Keine");   onTriggered: app.bulkSetPriority(contextMenu.targets, "") }
        }
        QQC2.Menu {
            title: i18n("Fälligkeit setzen")
            icon.name: "view-calendar-upcoming-days"
            QQC2.MenuItem { text: i18n("Heute");    onTriggered: app.bulkSetDue(contextMenu.targets, app.parseDueToken("today")) }
            QQC2.MenuItem { text: i18n("Morgen");   onTriggered: app.bulkSetDue(contextMenu.targets, app.parseDueToken("tomorrow")) }
            QQC2.MenuItem { text: i18n("+3 Tage");  onTriggered: app.bulkSetDue(contextMenu.targets, app.parseDueToken("+3d")) }
            QQC2.MenuItem { text: i18n("+1 Woche"); onTriggered: app.bulkSetDue(contextMenu.targets, app.parseDueToken("+1w")) }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { text: i18n("Keine");    onTriggered: app.bulkSetDue(contextMenu.targets, 0) }
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            text: i18n("Löschen …")
            icon.name: "edit-delete"
            onTriggered: {
                page.selection = contextMenu.targets
                deleteConfirm.open()
            }
        }
    }

    // ── Dialoge ─────────────────────────────────────────────────────────────
    Kirigami.PromptDialog {
        id: deleteConfirm
        title: i18n("Aufgaben löschen")
        subtitle: page.selection.length === 1
                  ? i18n("Die ausgewählte Aufgabe wirklich löschen?")
                  : i18n("%1 Aufgaben wirklich löschen?", page.selection.length)
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Löschen")
                icon.name: "edit-delete"
                onTriggered: {
                    app.deleteTasks(page.selection)
                    page.clearSelection()
                    deleteConfirm.close()
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: saveSearchPrompt
        title: i18n("Suche speichern")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel

        function openPrompt() {
            saveSearchField.text = app.searchQuery
            open()
            saveSearchField.forceActiveFocus()
        }

        QQC2.TextField {
            id: saveSearchField
            Layout.fillWidth: true
            placeholderText: i18n("Name der Suche")
            onAccepted: saveSearchPrompt.accept()
        }

        onAccepted: app.saveCurrentSearch(saveSearchField.text)
    }

    Kirigami.PromptDialog {
        id: newProjectPrompt
        title: i18n("Neues Projekt")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        property var targets: []

        function openFor(t) {
            targets = t
            newProjectField.text = ""
            open()
            newProjectField.forceActiveFocus()
        }

        QQC2.TextField {
            id: newProjectField
            Layout.fillWidth: true
            placeholderText: i18n("Projektname")
            onAccepted: newProjectPrompt.accept()
        }

        onAccepted: {
            if (newProjectField.text.trim().length > 0)
                app.bulkSetProject(targets, newProjectField.text.trim())
        }
    }

    Kirigami.PromptDialog {
        id: newTagPrompt
        title: i18n("Neuer Tag")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        property var targets: []

        function openFor(t) {
            targets = t
            newTagField.text = ""
            open()
            newTagField.forceActiveFocus()
        }

        QQC2.TextField {
            id: newTagField
            Layout.fillWidth: true
            placeholderText: i18n("Tag (ohne Leerzeichen)")
            onAccepted: newTagPrompt.accept()
        }

        onAccepted: {
            if (newTagField.text.trim().length > 0)
                app.bulkAddTag(targets, newTagField.text.trim())
        }
    }

    // ── Seiten-Shortcuts ────────────────────────────────────────────────────
    Shortcut {
        sequence: StandardKey.Find
        onActivated: searchField.forceActiveFocus()
    }
    Shortcut {
        sequence: "Ctrl+Shift+D"
        enabled: app.searchQuery.trim().length > 0
        onActivated: saveSearchPrompt.openPrompt()
    }
    Shortcut {
        sequence: "Return"
        enabled: page.selection.length === 1 && !searchField.activeFocus
        onActivated: root.openDetail(page.selection[0])
    }
    Shortcut {
        sequence: StandardKey.SelectAll
        enabled: !searchField.activeFocus
        onActivated: page.selection = Array.from(app.visibleUuids(0, taskList.count - 1))
    }
    Shortcut {
        sequence: "Escape"
        enabled: page.selection.length > 0
        onActivated: page.clearSelection()
    }
}
