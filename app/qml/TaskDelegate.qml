import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Eine Task-Zeile: Erledigt-Checkbox, ID-Chip, Titel (durchgestrichen wenn
// erledigt), Meta-Chips (Projekt, Tags, Fällig, Geplant, Wartet, Wiederholung,
// Blockiert, Notizen). Mehrfachauswahl per Klick/Strg/Umschalt, Drag auf die
// Sidebar, Kontextmenü, Doppelklick öffnet den Detail-Editor.
QQC2.ItemDelegate {
    id: delegate

    required property int index
    required property string uuid
    required property int wsId
    required property string title
    required property string project
    required property string tagsJson
    required property double due
    required property double scheduled
    required property double wait
    required property double start
    required property string priority
    required property string recur
    required property string statusKey
    required property bool isBlocked
    required property bool isBlocking
    required property int annotationCount

    readonly property bool completed: statusKey === "completed"

    // Zielpunkt der Erledigt-Checkbox in Fensterkoordinaten (für --test-input).
    function checkboxPoint() {
        return doneBox.mapToItem(null, doneBox.width / 2, doneBox.height / 2)
    }
    readonly property bool recurringMaster: statusKey === "recurring"
    readonly property var tags: JSON.parse(tagsJson || "[]")
    readonly property double nowSecs: Date.now() / 1000
    readonly property bool overdue: due > 0 && due < nowSecs && !completed

    width: ListView.view ? ListView.view.width : implicitWidth
    highlighted: page.isSelected(uuid)

    down: false // Klick-Feedback stört bei Mehrfachauswahl

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        QQC2.CheckBox {
            id: doneBox
            checked: delegate.completed
            visible: !delegate.recurringMaster
            onToggled: {
                if (checked)
                    app.markDone(delegate.uuid)
                else
                    app.reactivateTask(delegate.uuid)
            }
            QQC2.ToolTip.text: delegate.completed ? i18n("Reaktivieren") : i18n("Erledigt")
            QQC2.ToolTip.visible: hovered
        }

        Kirigami.Icon {
            visible: delegate.recurringMaster
            source: "task-recurring"
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
        }

        QQC2.Label {
            visible: delegate.wsId >= 0
            text: "#" + delegate.wsId
            opacity: 0.5
            font: Kirigami.Theme.smallFont
            Layout.minimumWidth: Kirigami.Units.gridUnit * 1.5
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            QQC2.Label {
                Layout.fillWidth: true
                text: delegate.title
                elide: Text.ElideRight
                font.strikeout: delegate.completed
                opacity: delegate.completed ? 0.6 : 1
            }

            Flow {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                visible: delegate.project.length > 0 || delegate.tags.length > 0
                         || delegate.due > 0 || delegate.scheduled > 0 || delegate.wait > 0
                         || delegate.recur.length > 0 || delegate.priority.length > 0
                         || delegate.isBlocked || delegate.isBlocking
                         || delegate.annotationCount > 0

                // Rot ist für Überfällig reserviert — Priorität signalisiert
                // über die Akzentfarbe (H) bzw. neutral (M/L), mit Klartext.
                MetaChip {
                    visible: delegate.priority.length > 0
                    iconName: "emblem-important"
                    label: delegate.priority === "H" ? i18n("Hoch")
                          : delegate.priority === "M" ? i18n("Mittel") : i18n("Niedrig")
                    chipColor: delegate.priority === "H" ? Kirigami.Theme.highlightColor
                              : Kirigami.Theme.textColor
                }
                MetaChip {
                    visible: delegate.project.length > 0
                    iconName: "folder"
                    label: delegate.project
                }
                Repeater {
                    model: delegate.tags.slice(0, 2)
                    MetaChip {
                        required property var modelData
                        iconName: "tag"
                        label: modelData
                    }
                }
                // Mehr als zwei Tags: bündeln, Details stehen im Editor.
                MetaChip {
                    visible: delegate.tags.length > 2
                    iconName: "tag"
                    label: i18n("+%1", delegate.tags.length - 2)
                }
                MetaChip {
                    visible: delegate.due > 0
                    iconName: "view-calendar-upcoming-days"
                    label: Qt.formatDate(new Date(delegate.due * 1000), Locale.ShortFormat)
                    chipColor: delegate.overdue ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                }
                MetaChip {
                    visible: delegate.scheduled > 0
                    iconName: "view-calendar"
                    label: i18n("Geplant ab %1", Qt.formatDate(new Date(delegate.scheduled * 1000), Locale.ShortFormat))
                }
                MetaChip {
                    visible: delegate.wait > 0 && delegate.wait > delegate.nowSecs
                    iconName: "clock"
                    label: i18n("Wartet bis %1", Qt.formatDate(new Date(delegate.wait * 1000), Locale.ShortFormat))
                }
                MetaChip {
                    visible: delegate.recur.length > 0
                    iconName: "task-recurring"
                    label: delegate.recur
                }
                // CLI-verwaltete Recurrence-Vorlage: nicht direkt erledigbar,
                // Instanzen erzeugt die Taskwarrior-CLI.
                MetaChip {
                    visible: delegate.recurringMaster
                    iconName: "view-refresh"
                    label: i18n("Vorlage (CLI-verwaltet)")
                    chipColor: Kirigami.Theme.neutralTextColor
                }
                MetaChip {
                    visible: delegate.start > 0
                    iconName: "media-playback-start"
                    label: i18n("Aktiv")
                    chipColor: Kirigami.Theme.positiveTextColor
                }
                MetaChip {
                    visible: delegate.isBlocked
                    iconName: "media-playback-paused"
                    label: i18n("Blockiert")
                    chipColor: Kirigami.Theme.neutralTextColor
                }
                MetaChip {
                    visible: delegate.isBlocking
                    iconName: "media-playback-playing"
                    label: i18n("Blockierend")
                }
                MetaChip {
                    visible: delegate.annotationCount > 0
                    iconName: "note"
                    label: delegate.annotationCount
                }
            }
        }
    }

    // Auswahl-Logik: Klick = Einzelauswahl, Strg = Umschalten, Umschalt = Bereich,
    // Doppelklick = Detail, Rechtsklick = Kontextmenü. Als MouseArea-Overlay
    // OBERHALB des Delegate-Buttons — der Button würde sonst den Links-Klick-Grab
    // übernehmen und TapHandler nie feuern lassen (von --test-input aufgedeckt).
    // Die Checkbox links bleibt ausgespart und damit nativ klickbar.
    MouseArea {
        anchors.fill: parent
        anchors.leftMargin: delegate.recurringMaster
                            ? 0
                            : doneBox.x + doneBox.width + delegate.leftPadding
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) {
                contextMenu.popupFor(delegate.uuid, delegate.completed)
            } else if (mouse.modifiers & Qt.ControlModifier) {
                page.toggleSelection(delegate.uuid, delegate.index)
            } else if (mouse.modifiers & Qt.ShiftModifier) {
                page.selectRange(delegate.index)
            } else {
                page.selectSingle(delegate.uuid, delegate.index)
            }
        }
        onDoubleClicked: mouse => {
            if (mouse.button === Qt.LeftButton && mouse.modifiers === Qt.NoModifier)
                root.openDetail(delegate.uuid)
        }
    }

    // Drag auf die Sidebar (Projekt/Tag/Eingang).
    Drag.dragType: Drag.Automatic
    Drag.active: dragHandler.active
    Drag.keys: ["application/x-vmn-tasks"]
    Drag.mimeData: ({ "application/x-vmn-tasks": delegate.uuid })

    DragHandler {
        id: dragHandler
        acceptedButtons: Qt.LeftButton
        onActiveChanged: {
            if (active) {
                root.dragUuids = page.isSelected(delegate.uuid)
                                 ? page.selection.slice()
                                 : [delegate.uuid]
            }
        }
    }

    // Kleiner Meta-Chip (Icon + Text) für die zweite Zeile.
    component MetaChip: Rectangle {
        property string iconName: ""
        property alias label: chipLabel.text
        property color chipColor: Kirigami.Theme.textColor

        radius: height / 2
        color: Qt.alpha(chipColor, 0.1)
        border.color: Qt.alpha(chipColor, 0.25)
        border.width: 1
        implicitWidth: chipRow.implicitWidth + Kirigami.Units.smallSpacing * 2
        implicitHeight: chipRow.implicitHeight + 2

        RowLayout {
            id: chipRow
            anchors.centerIn: parent
            spacing: 2

            Kirigami.Icon {
                source: iconName
                visible: iconName.length > 0
                color: chipColor
                Layout.preferredWidth: Kirigami.Units.iconSizes.small * 0.75
                Layout.preferredHeight: Kirigami.Units.iconSizes.small * 0.75
            }
            QQC2.Label {
                id: chipLabel
                font: Kirigami.Theme.smallFont
                color: chipColor
            }
        }
    }
}
