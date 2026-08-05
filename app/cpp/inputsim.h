#pragma once

class QString;

// Synthetische Eingabe-Events für den --test-input-Lauf: injiziert Maus- und
// Tastatur-Events direkt in das erste QQuickWindow (funktioniert offscreen,
// ohne Compositor und ohne echte Eingabegeräte).
void vmnSendClick(double x, double y, int button, int modifiers, bool doubleClick);
void vmnSendKey(int key, int modifiers, const QString &text);
// Hover-Position setzen (MouseMove ohne Taste): Klicks aktualisieren die
// QtQuick-Hover-Verfolgung nicht — für Screenshots muss der Hover gezielt
// von Tooltip-tragenden Elementen weggezogen werden.
void vmnSendMove(double x, double y);
