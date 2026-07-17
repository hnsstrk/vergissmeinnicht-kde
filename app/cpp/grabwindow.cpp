#include "grabwindow.h"

#include <QGuiApplication>
#include <QImage>
#include <QQuickWindow>

// Grabbt das zuletzt erzeugte sichtbare QQuickWindow: ist ein Formular-Fenster
// (Detail/Schnelleingabe/Einstellungen) geöffnet, wird es aufgenommen, sonst
// das Hauptfenster — dieselbe Zielwahl wie im Input-Shim.
bool vmnGrabFirstWindow(const QString &path)
{
    QQuickWindow *target = nullptr;
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow *w : windows) {
        if (auto *quickWindow = qobject_cast<QQuickWindow *>(w)) {
            if (quickWindow->isVisible()) {
                target = quickWindow;
            }
        }
    }
    if (!target) {
        return false;
    }
    const QImage image = target->grabWindow();
    return image.save(path);
}
