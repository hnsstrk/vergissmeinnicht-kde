import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Kategorie „KI-Assistent" der Einstellungen (UI-4): vorerst Platzhalter —
// die eigentliche KI-Konfiguration (Provider, Modellauswahl, API-Key, STT)
// füllt Story AI-A4 (#12).
FormCard.FormCardPage {
    id: page

    required property var app

    title: i18n("KI-Assistent")

    FormCard.FormCard {
        Layout.topMargin: Kirigami.Units.largeSpacing

        FormCard.FormTextDelegate {
            text: i18n("KI-Assistent")
            description: i18n("Die KI-Einstellungen (Provider, Modell, Spracheingabe) folgen mit der KI-Integration.")
        }
    }
}
