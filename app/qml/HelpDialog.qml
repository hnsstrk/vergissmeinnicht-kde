import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Hilfe: Tastenkürzel und Suchoperatoren (Pendant zu ⌘?-Sheet + Suche-Hilfe).
Kirigami.Dialog {
    id: dialog

    title: i18n("Hilfe")
    standardButtons: Kirigami.Dialog.Close
    preferredWidth: Kirigami.Units.gridUnit * 28
    maximumHeight: root.height - Kirigami.Units.gridUnit * 4

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            level: 3
            text: i18n("Tastenkürzel")
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.bottomMargin: 0
        }

        GridLayout {
            columns: 2
            columnSpacing: Kirigami.Units.gridUnit
            rowSpacing: Kirigami.Units.smallSpacing
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.topMargin: 0

            QQC2.Label { text: "Strg+N" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Neue Aufgabe") }
            QQC2.Label { text: "Strg+F" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Suche fokussieren") }
            QQC2.Label { text: "Strg+Z" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Letzte Änderung rückgängig") }
            QQC2.Label { text: "Strg+D" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Auswahl erledigen") }
            QQC2.Label { text: i18n("Entf") ; font.family: "monospace" }
            QQC2.Label { text: i18n("Auswahl löschen (mit Rückfrage)") }
            QQC2.Label { text: i18n("Eingabe") ; font.family: "monospace" }
            QQC2.Label { text: i18n("Details der ausgewählten Aufgabe") }
            QQC2.Label { text: "Strg+A" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Alle sichtbaren auswählen") }
            QQC2.Label { text: "Strg+Umschalt+D" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Suche speichern") }
            QQC2.Label { text: "Strg+Umschalt+H" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Erledigte ein-/ausblenden") }
            QQC2.Label { text: "Strg+Umschalt+S" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Synchronisieren") }
            QQC2.Label { text: "F5" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Aktualisieren") }
            QQC2.Label { text: "Strg+Umschalt+," ; font.family: "monospace" }
            QQC2.Label { text: i18n("Einstellungen") }
            QQC2.Label { text: "Esc" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Dialogfenster schließen / Auswahl aufheben") }
            QQC2.Label { text: i18n("Doppelklick") ; font.family: "monospace" }
            QQC2.Label { text: i18n("Details öffnen") }
            QQC2.Label { text: i18n("Strg/Umschalt+Klick") ; font.family: "monospace" }
            QQC2.Label { text: i18n("Mehrfachauswahl (einzeln / Bereich)") }
        }

        Kirigami.Separator { Layout.fillWidth: true }

        Kirigami.Heading {
            level: 3
            text: i18n("Suchfunktion")
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.bottomMargin: 0
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.topMargin: 0
            wrapMode: Text.WordWrap
            text: i18n("Die Suche durchsucht Titel, Projekt, Tags und Notizen über den gesamten Bestand (auch Erledigte). Mehrere Begriffe sind UND-verknüpft; Phrasen in doppelten Anführungszeichen (\"zwei worte\") werden zusammen gesucht. Bei aktiver Suche wird der Seitenleisten-Filter ignoriert.")
        }

        GridLayout {
            columns: 3
            columnSpacing: Kirigami.Units.gridUnit
            rowSpacing: Kirigami.Units.smallSpacing
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.topMargin: 0

            QQC2.Label { text: i18n("Operator"); font.bold: true }
            QQC2.Label { text: i18n("Funktion"); font.bold: true }
            QQC2.Label { text: i18n("Beispiel"); font.bold: true }

            QQC2.Label { text: "projekt: / project:" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Nach Projekt filtern") }
            QQC2.Label { text: "projekt:Büro" ; font.family: "monospace" }

            QQC2.Label { text: "tag:" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Nach Tag filtern") }
            QQC2.Label { text: "tag:dringend" ; font.family: "monospace" }

            QQC2.Label { text: "status:" ; font.family: "monospace" }
            QQC2.Label { text: i18n("offen · erledigt · wiederkehrend") }
            QQC2.Label { text: "status:erledigt" ; font.family: "monospace" }

            QQC2.Label { text: "+TAG" ; font.family: "monospace" }
            QQC2.Label {
                text: i18n("Virtuelle Tags (GROSS): OVERDUE, ACTIVE, BLOCKED, DUE, TODAY, WEEK, TAGGED …")
                wrapMode: Text.WordWrap
                Layout.maximumWidth: Kirigami.Units.gridUnit * 12
            }
            QQC2.Label { text: "+OVERDUE" ; font.family: "monospace" }

            QQC2.Label { text: "due.before: / due.after:" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Fälligkeit vor/nach Datum (auch eow, friday, 23rd …)") }
            QQC2.Label { text: "due.before:eom" ; font.family: "monospace" }

            QQC2.Label { text: "project.not:" ; font.family: "monospace" }
            QQC2.Label { text: i18n("Projekt ausschließen") }
            QQC2.Label { text: "project.not:privat" ; font.family: "monospace" }
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.topMargin: 0
            wrapMode: Text.WordWrap
            opacity: 0.8
            text: i18n("Tipp: Eine aktive Suche lässt sich über das Lesezeichen-Symbol (oder Strg+Umschalt+D) als „Gespeicherte Suche“ in die Seitenleiste legen.")
        }

        Kirigami.Separator { Layout.fillWidth: true }

        Kirigami.Heading {
            level: 3
            text: i18n("Schnelleingabe-Syntax")
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.bottomMargin: 0
        }

        QQC2.Label {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.topMargin: 0
            wrapMode: Text.WordWrap
            font.family: "monospace"
            text: "+tag   project:Name   due:today|tomorrow|+3d|+2w|2026-12-31   priority:H|M|L"
        }
    }
}
