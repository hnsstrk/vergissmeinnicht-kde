#pragma once

class QString;

// Rendert das erste Top-Level-QQuickWindow synchron in eine Bilddatei.
// Für Screenshot-/Verifikations-Läufe (--test-grab) — funktioniert auch,
// wenn der Compositor keine Frame-Callbacks liefert (gesperrte Session).
bool vmnGrabFirstWindow(const QString &path);
