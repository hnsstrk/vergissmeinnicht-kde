import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Persistente Seitenleiste (nicht-modaler OverlayDrawer, Merkuro-Muster):
// Systemfilter → Gespeicherte Suchen → Projekte → Tags, jeweils mit Zählern.
// Projekt-/Tag-/Eingang-Zeilen sind Drop-Ziele für Task-Drags.
Kirigami.OverlayDrawer {
    id: drawer

    edge: Qt.application.layoutDirection === Qt.RightToLeft ? Qt.RightEdge : Qt.LeftEdge
    modal: false
    closePolicy: QQC2.Popup.NoAutoClose
    width: app.sidebarWidth > 0 ? app.sidebarWidth : Kirigami.Units.gridUnit * 13
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0

    // Eingeklappte Projekt-Knoten (Name → true); Kinder eingeklappter Knoten
    // werden ausgeblendet.
    property var collapsedProjects: ({})

    // Eingeklappte Sektionen ("saved" | "projects" | "tags") — persistiert.
    property var collapsedSections: ({})

    Component.onCompleted: {
        const keys = JSON.parse(app.collapsedSectionsJson || "[]")
        const c = {}
        for (const k of keys)
            c[k] = true
        collapsedSections = c
    }

    function toggleSection(key) {
        const c = Object.assign({}, collapsedSections)
        if (c[key])
            delete c[key]
        else
            c[key] = true
        collapsedSections = c
        app.setCollapsedSectionsSetting(JSON.stringify(Object.keys(c)))
    }

    function projectVisible(name) {
        const parts = name.split(".")
        for (let i = 1; i < parts.length; i++) {
            if (collapsedProjects[parts.slice(0, i).join(".")])
                return false
        }
        return true
    }

    // Systemfilter — Reihenfolge wie macOS-Sidebar; "Wartend" nur bei Bedarf.
    readonly property var systemFilters: [
        { key: "inbox", label: i18n("Eingang"), icon: "mail-folder-inbox" },
        { key: "today", label: i18n("Heute"), icon: "go-jump-today" },
        { key: "todo", label: i18n("Zu erledigen"), icon: "view-task" },
        { key: "overdue", label: i18n("Überfällig"), icon: "appointment-missed" },
        { key: "duesoon", label: i18n("Bald fällig"), icon: "view-calendar-upcoming-days" },
        { key: "upcoming", label: i18n("Geplant"), icon: "view-calendar" },
        { key: "waiting", label: i18n("Wartend"), icon: "clock" },
        { key: "all", label: i18n("Alle"), icon: "view-list-details" }
    ]

    contentItem: Item {
        QQC2.ScrollView {
            id: sidebarScroll
            anchors.fill: parent
            QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

            ColumnLayout {
                // availableWidth statt drawer.width: lässt dem (nicht
                // transienten) Scrollbalken seinen Platz, statt die Zähler
                // darunter zu begraben.
                width: sidebarScroll.availableWidth
                spacing: 0

                Repeater {
                    model: drawer.systemFilters
                    delegate: SidebarRow {
                        required property var modelData
                        filterKey: modelData.key
                        label: modelData.label
                        iconName: modelData.icon
                        count: root.counts[modelData.key] ?? 0
                        visible: modelData.key !== "waiting" || count > 0
                        acceptsDrop: modelData.key === "inbox"
                        onDropped: uuids => app.dropOnInbox(uuids)
                    }
                }

                SectionHeader {
                    sectionKey: "saved"
                    text: i18n("Gespeicherte Suchen")
                    visible: root.savedSearches.length > 0
                }

                Repeater {
                    model: root.savedSearches
                    delegate: SidebarRow {
                        required property var modelData
                        filterKey: "saved:" + modelData.id
                        label: modelData.name
                        iconName: "bookmarks"
                        count: -1
                        visible: !drawer.collapsedSections["saved"]
                        contextMenu: savedMenu
                        onOpenContextMenu: {
                            savedMenu.searchId = modelData.id
                            savedMenu.searchName = modelData.name
                            savedMenu.popup()
                        }
                    }
                }

                SectionHeader {
                    sectionKey: "projects"
                    text: i18n("Projekte")
                    visible: root.projects.length > 0
                }

                Repeater {
                    model: root.projects
                    delegate: SidebarRow {
                        required property var modelData
                        filterKey: "project:" + modelData.name
                        label: modelData.label ?? modelData.name
                        iconName: "folder"
                        count: modelData.count
                        depth: modelData.depth ?? 0
                        hasChildren: modelData.hasChildren ?? false
                        expanded: !drawer.collapsedProjects[modelData.name]
                        visible: !drawer.collapsedSections["projects"] && drawer.projectVisible(modelData.name)
                        acceptsDrop: true
                        onDropped: uuids => app.dropOnProject(uuids, modelData.name)
                        onToggleExpanded: {
                            const c = Object.assign({}, drawer.collapsedProjects)
                            if (c[modelData.name])
                                delete c[modelData.name]
                            else
                                c[modelData.name] = true
                            drawer.collapsedProjects = c
                        }
                        onOpenContextMenu: {
                            projectMenu.projectName = modelData.name
                            projectMenu.popup()
                        }
                    }
                }

                SectionHeader {
                    sectionKey: "tags"
                    text: i18n("Tags")
                    visible: root.tagList.length > 0
                }

                Repeater {
                    model: root.tagList
                    delegate: SidebarRow {
                        required property var modelData
                        filterKey: "tag:" + modelData.name
                        label: modelData.name
                        iconName: "tag"
                        count: modelData.count
                        visible: !drawer.collapsedSections["tags"]
                        acceptsDrop: true
                        onDropped: uuids => app.dropOnTag(uuids, modelData.name)
                        onOpenContextMenu: {
                            tagMenu.tagName = modelData.name
                            tagMenu.popup()
                        }
                    }
                }

                Item { Layout.fillHeight: true }
            }
        }

        // Zieh-Griff am Rand: Sidebar-Breite anpassen, Wert wird persistiert.
        // Die imperative width-Zuweisung beim Ziehen ersetzt das deklarative
        // Binding von oben dauerhaft — gewollt: ab dann gilt der Nutzerwert,
        // beim nächsten Start greift das Binding mit dem persistierten Wert.
        MouseArea {
            width: Kirigami.Units.smallSpacing * 2
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            x: drawer.edge === Qt.RightEdge ? 0 : parent.width - width
            cursorShape: Qt.SplitHCursor
            acceptedButtons: Qt.LeftButton
            preventStealing: true
            property real pressX: 0
            onPressed: mouse => pressX = mouse.x
            onPositionChanged: mouse => {
                if (!pressed)
                    return
                const delta = mouse.x - pressX
                const target = drawer.edge === Qt.RightEdge ? drawer.width - delta : drawer.width + delta
                const min = Kirigami.Units.gridUnit * 8
                const max = Math.min(Kirigami.Units.gridUnit * 30, root.width / 2)
                drawer.width = Math.max(min, Math.min(max, target))
            }
            onReleased: app.setSidebarWidthSetting(Math.round(drawer.width))
        }
    }

    // ── Kontextmenüs ────────────────────────────────────────────────────────

    QQC2.Menu {
        id: projectMenu
        property string projectName: ""
        QQC2.MenuItem {
            text: i18n("Umbenennen …")
            icon.name: "edit-rename"
            onTriggered: renamePrompt.openFor(i18n("Projekt umbenennen"), projectMenu.projectName,
                                              name => app.renameProject(projectMenu.projectName, name))
        }
        QQC2.MenuItem {
            text: i18n("Aus allen Aufgaben entfernen")
            icon.name: "edit-delete-remove"
            onTriggered: app.clearProject(projectMenu.projectName)
        }
    }

    QQC2.Menu {
        id: tagMenu
        property string tagName: ""
        QQC2.MenuItem {
            text: i18n("Umbenennen …")
            icon.name: "edit-rename"
            onTriggered: renamePrompt.openFor(i18n("Tag umbenennen"), tagMenu.tagName,
                                              name => app.renameTag(tagMenu.tagName, name))
        }
        QQC2.MenuItem {
            text: i18n("Aus allen Aufgaben entfernen")
            icon.name: "edit-delete-remove"
            onTriggered: app.clearTag(tagMenu.tagName)
        }
    }

    QQC2.Menu {
        id: savedMenu
        property string searchId: ""
        property string searchName: ""
        QQC2.MenuItem {
            text: i18n("Umbenennen …")
            icon.name: "edit-rename"
            onTriggered: renamePrompt.openFor(i18n("Suche umbenennen"), savedMenu.searchName,
                                              name => app.renameSavedSearch(savedMenu.searchId, name))
        }
        QQC2.MenuItem {
            text: i18n("Löschen")
            icon.name: "edit-delete"
            onTriggered: app.deleteSavedSearch(savedMenu.searchId)
        }
    }

    Kirigami.PromptDialog {
        id: renamePrompt
        property var callback: null
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel

        function openFor(promptTitle, currentValue, cb) {
            title = promptTitle
            renameField.text = currentValue
            callback = cb
            open()
            renameField.forceActiveFocus()
        }

        QQC2.TextField {
            id: renameField
            Layout.fillWidth: true
            onAccepted: renamePrompt.accept()
        }

        onAccepted: {
            if (callback && renameField.text.trim().length > 0)
                callback(renameField.text.trim())
        }
    }

    // ── Sektions-Überschrift (klickbar: Sektion ein-/ausklappen) ────────────

    component SectionHeader: Kirigami.ListSectionHeader {
        id: header

        property string sectionKey: ""

        Layout.fillWidth: true
        // ListSectionHeader ist als reines Label gebaut — für die Klapp-
        // Interaktion wieder hover- und tastaturbedienbar machen.
        hoverEnabled: true
        activeFocusOnTab: true
        onClicked: drawer.toggleSection(sectionKey)

        Kirigami.Icon {
            source: drawer.collapsedSections[header.sectionKey] ? "arrow-right" : "arrow-down"
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            Layout.rightMargin: Kirigami.Units.largeSpacing
        }
    }

    // ── Zeilen-Komponente ───────────────────────────────────────────────────

    component SidebarRow: QQC2.ItemDelegate {
        id: row

        property string filterKey: ""
        property string label: ""
        property string iconName: ""
        property int count: 0
        property bool acceptsDrop: false
        property var contextMenu: null
        // Hierarchie (Projekte): Einrück-Tiefe + Klapp-Zustand.
        property int depth: 0
        property bool hasChildren: false
        property bool expanded: true

        signal dropped(var uuids)
        signal openContextMenu()
        signal toggleExpanded()

        Layout.fillWidth: true
        text: label
        icon.name: iconName
        highlighted: app.filterKey === filterKey
        onClicked: app.applyFilter(filterKey)

        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Item { Layout.preferredWidth: row.depth * Kirigami.Units.gridUnit; visible: row.depth > 0 }
            QQC2.ToolButton {
                visible: row.hasChildren
                icon.name: row.expanded ? "arrow-down" : "arrow-right"
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                onClicked: row.toggleExpanded()
            }
            Kirigami.Icon {
                source: row.iconName
                Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            }
            QQC2.Label {
                Layout.fillWidth: true
                text: row.label
                elide: Text.ElideRight
            }
            QQC2.Label {
                visible: row.count > 0
                text: row.count
                opacity: 0.6
                // Abstand zum Rand und zum (unsichtbaren) Resize-Griff.
                Layout.rightMargin: Kirigami.Units.largeSpacing
            }
        }

        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: row.openContextMenu()
        }

        DropArea {
            anchors.fill: parent
            enabled: row.acceptsDrop
            keys: ["application/x-vmn-tasks"]
            onEntered: row.highlighted = true
            onExited: row.highlighted = Qt.binding(() => app.filterKey === row.filterKey)
            onDropped: {
                row.highlighted = Qt.binding(() => app.filterKey === row.filterKey)
                if (root.dragUuids.length > 0)
                    row.dropped(root.dragUuids)
            }
        }
    }
}
