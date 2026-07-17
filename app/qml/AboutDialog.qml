import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.Dialog {
    id: dialog

    title: i18n("Über Vergissmeinnicht")
    standardButtons: Kirigami.Dialog.Close
    preferredWidth: Kirigami.Units.gridUnit * 22

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: "de.hnsstrk.vergissmeinnicht"
            fallback: "view-task"
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.preferredWidth: Kirigami.Units.iconSizes.enormous
            Layout.preferredHeight: Kirigami.Units.iconSizes.enormous
        }

        Kirigami.Heading {
            text: "Vergissmeinnicht"
            level: 1
            Layout.alignment: Qt.AlignHCenter
        }

        QQC2.Label {
            text: i18n("Version %1", Qt.application.version.length > 0 ? Qt.application.version : "0.1.0")
            opacity: 0.7
            Layout.alignment: Qt.AlignHCenter
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: i18n("Nativer KDE-Client für Taskwarrior 3.x auf Basis von TaskChampion.\nKirigami-Oberfläche, Rust-Kern via cxx-qt.")
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.bottomMargin: Kirigami.Units.largeSpacing
            horizontalAlignment: Text.AlignHCenter
            textFormat: Text.RichText
            onLinkActivated: link => Qt.openUrlExternally(link)
            text: "<a href=\"https://github.com/hnsstrk/vergissmeinnicht-kde\">github.com/hnsstrk/vergissmeinnicht-kde</a> · MIT-Lizenz"
        }
    }
}
