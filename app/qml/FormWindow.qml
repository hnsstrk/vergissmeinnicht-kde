import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Eigenständiges Formular-Fenster (statt In-Fenster-Modal): echte Titelzeile
// des Fenstermanagers, frei beweg- und skalierbar, scrollender Inhalt,
// Aktionsleiste unten. Öffnen über openWindow(), Schließen über close().
Window {
    id: win

    flags: Qt.Dialog
    transientParent: root
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 14
    color: bgRect.color

    // Formularzeilen (default) und Fußleisten-Buttons.
    default property alias formContent: contentColumn.data
    property alias footer: footerRow.data

    function openWindow() {
        show()
        raise()
        requestActivate()
    }

    Shortcut {
        sequence: "Escape"
        onActivated: win.close()
    }

    Rectangle {
        id: bgRect
        anchors.fill: parent
        Kirigami.Theme.inherit: false
        Kirigami.Theme.colorSet: Kirigami.Theme.Window
        color: Kirigami.Theme.backgroundColor
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        QQC2.ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

            ColumnLayout {
                id: contentColumn
                // Luft zur Fensterkante — Formularzeilen kleben sonst randlos.
                width: scroll.availableWidth - Kirigami.Units.largeSpacing * 2
                x: Kirigami.Units.largeSpacing
                spacing: 0
            }
        }

        Kirigami.Separator { Layout.fillWidth: true }

        RowLayout {
            id: footerRow
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            // Spacer: die von Instanzen ergänzten Buttons sitzen rechts.
            Item { Layout.fillWidth: true }
        }
    }
}
