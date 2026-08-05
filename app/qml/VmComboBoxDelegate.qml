import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

// Gemeinsame ComboBox-Zeile der Einstellungsseiten (UI-7, #34): Die
// Addons-Vorlage FormComboBoxDelegate setzt ihrer internen QQC2.ComboBox
// keine Popup-Höhengrenze. Beim org.kde.desktop-Style ist das Popup ein
// Menu, das unter dem Feld aufklappt — wird es höher als der Platz darunter,
// schiebt es der Popup-Positionierer nach oben über das ganze Fenster
// (Kundenbefund: 15 Modelle verdecken die komplette Einstellungsseite).
// Dieser Delegate deckelt die Popup-Höhe auf rund zehn Einträge bzw. die
// Fensterhöhe und macht die Liste darüber hinaus scrollbar.
FormCard.FormComboBoxDelegate {
    id: wurzel

    // Höchstens so viele Einträge ohne Scrollen; darüber wird gescrollt.
    readonly property int maxSichtbareEintraege: 10

    // Die interne QQC2.ComboBox der Addons-Vorlage — sie ist dort nicht als
    // Alias exportiert, deshalb Suche über den Item-Baum (erkennbar an der
    // Eigenschafts-Kombination popup + editText, die nur ComboBox trägt).
    property Item interneCombo: null

    function findeCombo(item) {
        if (!item)
            return null
        for (let i = 0; i < item.children.length; ++i) {
            const kind = item.children[i]
            if (kind.popup !== undefined && kind.editText !== undefined)
                return kind
            const tiefer = findeCombo(kind)
            if (tiefer)
                return tiefer
        }
        return null
    }

    // Höhendeckel: anteilig aus der Inhaltshöhe (rund zehn Einträge), nie
    // höher, als das Fenster der Seite zulässt.
    function maxPopupHoehe(popup, anzahl) {
        const rand = popup.topPadding + popup.bottomPadding
        let deckel = popup.implicitHeight
        if (anzahl > maxSichtbareEintraege && popup.implicitHeight > rand)
            deckel = rand + (popup.implicitHeight - rand) * maxSichtbareEintraege / anzahl
        const fenster = Window.window
        if (fenster)
            deckel = Math.min(deckel, fenster.height - Kirigami.Units.gridUnit * 2)
        return deckel
    }

    Component.onCompleted: {
        interneCombo = findeCombo(wurzel.contentItem)
        if (!interneCombo) {
            // Obacht bei Addons-Updates: ohne Fund bleibt das Popup wie in
            // der Vorlage unbegrenzt — sichtbar machen statt still schlucken.
            console.warn("VmComboBoxDelegate: interne ComboBox nicht gefunden — Popup bleibt unbegrenzt")
            return
        }
        const combo = interneCombo
        const popup = combo.popup
        popup.height = Qt.binding(function() {
            return Math.min(popup.implicitHeight, wurzel.maxPopupHoehe(popup, combo.count))
        })
        // Der Style bindet interactive an „höher als das Fenster" — mit
        // Deckel muss die Liste schon scrollen, wenn sie höher als das
        // Popup ist (der Scrollbalken folgt von selbst).
        const liste = popup.contentItem
        if (liste && liste.contentHeight !== undefined)
            liste.interactive = Qt.binding(function() {
                return liste.contentHeight > liste.height + 1
            })
    }
}
