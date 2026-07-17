#include "inputsim.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QQuickWindow>

static QQuickWindow *firstQuickWindow()
{
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow *w : windows) {
        if (auto *quickWindow = qobject_cast<QQuickWindow *>(w)) {
            return quickWindow;
        }
    }
    return nullptr;
}

// Monoton steigender Fake-Timestamp — ohne ihn verwirft/fehlinterpretiert die
// QtQuick-Delivery synthetische Events (dasselbe macht QTest intern).
static ulong vmnNextTimestamp()
{
    static ulong timestamp = 1000;
    timestamp += 30;
    return timestamp;
}

void vmnSendClick(double x, double y, int button, int modifiers, bool doubleClick)
{
    QQuickWindow *w = firstQuickWindow();
    if (!w) {
        return;
    }
    const QPointF pos(x, y);
    const QPointF global = w->mapToGlobal(pos);
    const auto btn = static_cast<Qt::MouseButton>(button);
    const auto mods = static_cast<Qt::KeyboardModifiers>(static_cast<unsigned>(modifiers));

    auto send = [&](QEvent::Type type, Qt::MouseButtons buttons) {
        QMouseEvent event(type, pos, global, btn, buttons, mods);
        event.setTimestamp(vmnNextTimestamp());
        QCoreApplication::sendEvent(w, &event);
    };
    send(QEvent::MouseButtonPress, btn);
    send(QEvent::MouseButtonRelease, Qt::NoButton);
    if (doubleClick) {
        // Zweites Press/Release-Paar dicht dahinter — plus explizites
        // DblClick-Event für Empfänger, die den Event-Typ auswerten.
        send(QEvent::MouseButtonPress, btn);
        send(QEvent::MouseButtonDblClick, btn);
        send(QEvent::MouseButtonRelease, Qt::NoButton);
    }
}

void vmnSendKey(int key, int modifiers, const QString &text)
{
    QQuickWindow *w = firstQuickWindow();
    if (!w) {
        return;
    }
    const auto mods = static_cast<Qt::KeyboardModifiers>(static_cast<unsigned>(modifiers));
    QKeyEvent press(QEvent::KeyPress, key, mods, text);
    press.setTimestamp(vmnNextTimestamp());
    QCoreApplication::sendEvent(w, &press);
    QKeyEvent release(QEvent::KeyRelease, key, mods, text);
    release.setTimestamp(vmnNextTimestamp());
    QCoreApplication::sendEvent(w, &release);
}
