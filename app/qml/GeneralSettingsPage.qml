import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Kategorie „Allgemein" der Einstellungen (UI-4): Wortlaut und Verhalten
// unverändert aus dem alten Ein-Fenster-Dialog übernommen — Umbau, kein
// Redesign. Die AppContainer-Instanz kommt als initialProperty aus
// SettingsDialog.qml, weil die Seite per Qt.createComponent außerhalb des
// Main-Scopes entsteht und die `app`-Id dort nicht sieht.
FormCard.FormCardPage {
    id: page

    required property var app

    title: i18n("Allgemein")

    FormCard.FormCard {
        Layout.topMargin: Kirigami.Units.largeSpacing

        VmComboBoxDelegate {
            id: defaultFilterCombo
            text: i18n("Standardansicht beim Start")
            readonly property var keys: ["inbox", "today", "todo", "overdue", "duesoon", "upcoming", "all"]
            model: [i18n("Eingang"), i18n("Heute"), i18n("Zu erledigen"), i18n("Überfällig"), i18n("Bald fällig"), i18n("Geplant"), i18n("Alle")]
            currentIndex: Math.max(0, keys.indexOf(page.app.defaultFilter))
            onActivated: page.app.setDefaultFilterSetting(keys[currentIndex])
        }

        VmComboBoxDelegate {
            id: languageCombo
            text: i18n("Sprache")
            description: i18n("Änderung wird nach einem Neustart wirksam. Standard-Dialogknöpfe (OK/Abbrechen) folgen der Systemsprache.")
            readonly property var keys: ["", "de", "en"]
            model: [i18n("Systemsprache"), "Deutsch", "English"]
            currentIndex: Math.max(0, keys.indexOf(page.app.languageSetting()))
            onActivated: page.app.setLanguageSetting(keys[currentIndex])
        }

        FormCard.FormSpinBoxDelegate {
            label: i18n("„Bald fällig“-Fenster (Tage)")
            from: 1
            to: 60
            value: page.app.dueSoonDays
            onValueChanged: {
                if (value !== page.app.dueSoonDays)
                    page.app.setDueSoonDaysSetting(value)
            }
        }

        FormCard.FormSwitchDelegate {
            text: i18n("Erledigte Aufgaben ausblenden")
            checked: page.app.hideCompleted
            onToggled: page.app.setHideCompletedSetting(checked)
        }

        FormCard.FormSwitchDelegate {
            text: i18n("Benachrichtigung bei überfälligen Aufgaben")
            description: i18n("Zusammenfassung beim Programmstart, wenn überfällige Aufgaben vorliegen.")
            checked: page.app.notifyOverdue
            onToggled: page.app.setNotifyOverdueSetting(checked)
        }
    }
}
