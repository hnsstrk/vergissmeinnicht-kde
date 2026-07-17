#pragma once

class QQmlEngine;
class QString;

// Installiert einen KLocalizedContext (KF6 ki18n) als Kontext-Objekt der
// QML-Engine. Kirigami Addons (FormCard u.a.) setzen die i18nd*-Funktionen
// voraus; außerdem nutzt unser eigenes QML damit i18n() für Übersetzungen.
void vmnInstallKLocalizedContext(QQmlEngine &engine);

// Erzwingt eine UI-Sprache (z. B. "en" oder "de") unabhängig vom System-Locale.
// Leerer String = Systemsprache. Muss vor dem Laden des QML aufgerufen werden.
void vmnSetUiLanguage(const QString &language);
