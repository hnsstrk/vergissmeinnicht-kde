import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Kategorie „Diktat" der Einstellungen (#47): Spracherkennungs-Backend und
// Whisper-Modell bzw. -Pfade, herausgelöst aus der KI-Seite — Diktat und
// KI-Assistent sind unabhängig nutzbar, die Diktier-Sonde liest weder
// Basis-URL noch Modell. Gespeichert wird über „Speichern"; die Statuszeile
// meldet danach das Ergebnis der Diktier-Sonde (AI-A5, #41).
FormCard.FormCardPage {
    id: page

    required property var app

    title: i18n("Diktat")

    // Statuszeile: erst nach „Speichern" melden.
    property bool gespeichert: false

    Component.onCompleted: {
        const s = JSON.parse(app.aiSettingsJson())
        sttCombo.currentIndex = Math.max(0, sttCombo.keys.indexOf(s.ai_stt_backend))
        whisperModelField.text = s.ai_whisper_model
        whisperCppBinaryField.text = s.ai_whisper_cpp_binary
        whisperCppModelField.text = s.ai_whisper_cpp_model
    }

    FormCard.FormHeader {
        title: i18n("Spracheingabe (Diktat)")
    }

    FormCard.FormCard {
        VmComboBoxDelegate {
            id: sttCombo
            text: i18n("Spracherkennungs-Backend")
            readonly property var keys: ["openai-whisper", "whisper-cpp"]
            model: ["openai-whisper (CPU)", "whisper.cpp"]
        }

        FormCard.FormTextFieldDelegate {
            id: whisperModelField
            visible: sttCombo.currentIndex === 0
            label: i18n("Whisper-Modell")
            placeholderText: "small"
        }

        FormCard.FormTextFieldDelegate {
            id: whisperCppBinaryField
            visible: sttCombo.currentIndex === 1
            label: i18n("whisper-cli-Programm (Pfad)")
        }

        FormCard.FormTextFieldDelegate {
            id: whisperCppModelField
            visible: sttCombo.currentIndex === 1
            label: i18n("GGML-Modelldatei (Pfad)")
        }
    }

    FormCard.FormCard {
        Layout.topMargin: Kirigami.Units.largeSpacing

        FormCard.FormButtonDelegate {
            text: i18n("Speichern")
            description: i18n("Speichert die Diktat-Einstellungen und prüft die Diktier-Kette (Aufnahme und Spracherkennung).")
            icon.name: "dialog-ok-apply"
            onClicked: {
                page.app.saveDictationSettings(sttCombo.keys[sttCombo.currentIndex],
                                               whisperModelField.text,
                                               whisperCppBinaryField.text,
                                               whisperCppModelField.text)
                page.gespeichert = true
            }
        }

        // Direktes Feedback IM Dialog (wie die Statuszeile der KI-Seite):
        // die Diktier-Sonde läuft beim Speichern neu, ihr Ergebnis steht
        // in dictationAvailable/dictationUnavailableReason (#41).
        FormCard.FormTextDelegate {
            visible: page.gespeichert
            text: i18n("Status")
            description: page.app.dictationAvailable
                         ? i18n("Gespeichert — Diktat ist einsatzbereit.")
                         : i18n("Gespeichert. %1", page.app.dictationUnavailableReason)
            descriptionItem.color: page.app.dictationAvailable
                                   ? Kirigami.Theme.positiveTextColor
                                   : Kirigami.Theme.neutralTextColor
        }
    }
}
