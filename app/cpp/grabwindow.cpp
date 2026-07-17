#include "grabwindow.h"

#include <QGuiApplication>
#include <QImage>
#include <QQuickWindow>

bool vmnGrabFirstWindow(const QString &path)
{
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow *w : windows) {
        if (auto *quickWindow = qobject_cast<QQuickWindow *>(w)) {
            const QImage image = quickWindow->grabWindow();
            return image.save(path);
        }
    }
    return false;
}
