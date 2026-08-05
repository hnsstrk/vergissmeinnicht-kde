import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Kategorie „KI-Assistent" der Einstellungen (AI-A4, #12): Provider-Preset,
// Basis-URL, Modellauswahl aus der Modellliste des Endpunkts, API-Key
// (Secret Service) und Spracheingabe-Backend. Gespeichert wird über
// „Speichern und testen" bzw. „Modelle laden" — beide persistieren erst,
// weil der Llm-Client aus den gespeicherten Einstellungen gebaut wird.
FormCard.FormCardPage {
    id: page

    required property var app

    title: i18n("KI-Assistent")

    // Vom Endpunkt geladene Modellliste (aus aiModelsJson); leer, solange
    // nichts geladen wurde — die Combo bleibt dann frei editierbar.
    property var modelle: []
    // Statuszeile: erst nach „Speichern und testen" melden.
    property bool gespeichert: false

    // Lokale Endpunkte übertragen nichts nach außen — alles andere löst den
    // Datenschutzhinweis aus (leeres Feld zählt nicht als „entfernt").
    function istLokal(url) {
        return url.trim().length === 0
               || url.indexOf("localhost") !== -1
               || url.indexOf("127.0.0.1") !== -1
               || url.indexOf("[::1]") !== -1
    }

    // Persistiert alle Felder der Seite (Settings + API-Key im Secret
    // Service) und invalidiert damit Rust-seitig den gecachten Llm-Client.
    function speichern() {
        app.clearAiError()
        gespeichert = false
        app.saveAiSettings(providerCombo.keys[providerCombo.currentIndex],
                           baseUrlField.text,
                           modelCombo.editText,
                           sttCombo.keys[sttCombo.currentIndex],
                           whisperModelField.text,
                           whisperCppBinaryField.text,
                           whisperCppModelField.text,
                           contextCombo.keys[contextCombo.currentIndex])
        app.setAiApiKey(keyField.text)
    }

    Component.onCompleted: {
        app.clearAiError()
        const s = JSON.parse(app.aiSettingsJson())
        providerCombo.currentIndex = Math.max(0, providerCombo.keys.indexOf(s.ai_provider))
        baseUrlField.text = s.ai_base_url
        modelCombo.editText = s.ai_model
        keyField.text = app.aiApiKey()
        sttCombo.currentIndex = Math.max(0, sttCombo.keys.indexOf(s.ai_stt_backend))
        whisperModelField.text = s.ai_whisper_model
        whisperCppBinaryField.text = s.ai_whisper_cpp_binary
        whisperCppModelField.text = s.ai_whisper_cpp_model
        contextCombo.currentIndex = Math.max(0, contextCombo.keys.indexOf(s.ai_context_level))
    }

    // Geladene Modellliste in die Combo übernehmen. Das konfigurierte
    // Modell bleibt gewählt — auch wenn es nicht in der Liste steht
    // (Offline-Backend darf den Nutzer nicht aussperren).
    Connections {
        target: page.app
        function onAiModelsJsonChanged() {
            const gewaehlt = modelCombo.editText
            page.modelle = JSON.parse(page.app.aiModelsJson || "[]")
            const idx = page.modelle.indexOf(gewaehlt)
            if (idx >= 0)
                modelCombo.currentIndex = idx
            else
                modelCombo.editText = gewaehlt
        }
    }

    FormCard.FormHeader {
        title: i18n("KI-Backend")
    }

    FormCard.FormCard {
        FormCard.FormTextDelegate {
            text: i18n("OpenAI-kompatibler Endpunkt")
            description: i18n("Vorgabe ist Ollama auf dieser Maschine — dabei verlassen keine Daten den Rechner. Der API-Key wird im Passwortspeicher des Systems (KWallet/Secret Service) abgelegt.")
        }

        FormCard.FormComboBoxDelegate {
            id: providerCombo
            text: i18n("Provider")
            readonly property var keys: ["ollama", "openrouter", "custom"]
            model: [i18n("Ollama (lokal)"), i18n("OpenRouter (Cloud)"), i18n("Benutzerdefiniert")]
            onActivated: {
                // Preset befüllt die Basis-URL vor; „Benutzerdefiniert"
                // liefert leer und lässt das Feld unangetastet.
                const vorgabe = page.app.aiProviderDefaultUrl(keys[currentIndex])
                if (vorgabe.length > 0)
                    baseUrlField.text = vorgabe
            }
        }

        FormCard.FormTextFieldDelegate {
            id: baseUrlField
            label: i18n("Basis-URL (inklusive /v1)")
            placeholderText: "http://localhost:11434/v1"
        }

        // Kontextumfang der KI-Interpretation (AI-B1b, #31): drei Stufen —
        // die Beschreibung benennt je Stufe, was die Maschine verlässt.
        FormCard.FormComboBoxDelegate {
            id: contextCombo
            text: i18n("Kontextumfang der Interpretation")
            readonly property var keys: ["taxonomy", "open_titles", "all"]
            model: [i18n("Nur Projekt- und Schlagwortnamen"),
                    i18n("Zusätzlich Titel offener Aufgaben"),
                    i18n("Alle Aufgaben (kompakt)")]
            description: {
                switch (currentIndex) {
                case 1:
                    return i18n("Die KI erhält Freitext, Datum, Projekt- und Schlagwortnamen sowie die Titel aller offenen Aufgaben.")
                case 2:
                    return i18n("Die KI erhält Freitext, Datum, Projekt- und Schlagwortnamen sowie alle nicht gelöschten Aufgaben (Titel, Projekt, Schlagworte, Fälligkeit, Erledigt-Markierung).")
                default:
                    return i18n("Die KI erhält nur Freitext, Datum sowie vorhandene Projekt- und Schlagwortnamen — keine Aufgabentitel.")
                }
            }
        }

        // Datenschutz sichtbar machen (Spec §3.3): sobald der Endpunkt
        // nicht mehr lokal ist, wird die Übertragung benannt — bei den
        // Stufen B/C ausdrücklich samt Aufgabeninhalten (AI-B1b).
        FormCard.FormTextDelegate {
            visible: !page.istLokal(baseUrlField.text)
            text: i18n("Hinweis zum Datenschutz")
            description: contextCombo.currentIndex > 0
                         ? i18n("Die Basis-URL zeigt nicht auf localhost — Aufgabendaten werden an diesen Endpunkt übertragen. Beim gewählten Kontextumfang gehören dazu auch Titel und Metadaten Ihrer Aufgaben.")
                         : i18n("Die Basis-URL zeigt nicht auf localhost — Aufgabendaten werden an diesen Endpunkt übertragen.")
            descriptionItem.color: Kirigami.Theme.neutralTextColor
        }

        FormCard.FormComboBoxDelegate {
            id: modelCombo
            text: i18n("Modell")
            editable: true
            model: page.modelle
        }

        FormCard.FormButtonDelegate {
            text: i18n("Modelle laden")
            description: i18n("Speichert die Einstellungen und holt die Modellliste des Endpunkts. Ist das Backend nicht erreichbar, bleibt die manuelle Eingabe möglich.")
            icon.name: "view-refresh"
            enabled: !page.app.aiBusy
            onClicked: {
                page.speichern()
                page.app.startAiListModels()
            }
        }

        FormCard.FormPasswordFieldDelegate {
            id: keyField
            label: i18n("API-Key (nur für Cloud-Endpunkte)")
        }
    }

    FormCard.FormHeader {
        title: i18n("Spracheingabe (Diktat)")
    }

    FormCard.FormCard {
        FormCard.FormComboBoxDelegate {
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
            id: saveTestButton
            text: i18n("Speichern und testen")
            description: i18n("Speichert alle KI-Einstellungen und prüft die Verbindung über die Modellliste.")
            icon.name: "dialog-ok-apply"
            enabled: !page.app.aiBusy
            onClicked: {
                page.speichern()
                page.gespeichert = true
                page.app.startAiListModels()
            }
        }

        // Direktes Feedback IM Dialog — das Fehlerbanner der Hauptansicht
        // liegt hinter dem Einstellungsfenster (und KI-Fehler laufen ohnehin
        // über den eigenen aiError-Kanal).
        FormCard.FormTextDelegate {
            id: aiStatusLine
            visible: page.gespeichert || page.app.aiBusy || page.app.aiError.length > 0
            text: i18n("Status")
            description: {
                if (page.app.aiError.length > 0)
                    return page.app.aiError
                if (page.app.aiBusy)
                    return i18n("Prüfe Verbindung …")
                if (page.gespeichert && page.modelle.length > 0)
                    return i18np("Gespeichert — Verbindung erfolgreich, 1 Modell verfügbar.",
                                 "Gespeichert — Verbindung erfolgreich, %1 Modelle verfügbar.",
                                 page.modelle.length)
                if (page.gespeichert)
                    return i18n("Gespeichert.")
                return ""
            }
            descriptionItem.color: page.app.aiError.length > 0
                                   ? Kirigami.Theme.negativeTextColor
                                   : Kirigami.Theme.positiveTextColor
        }
    }
}
