#include "klocalized.h"

#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include <QQmlContext>
#include <QQmlEngine>

void vmnInstallKLocalizedContext(QQmlEngine &engine)
{
    KLocalizedString::setApplicationDomain("vergissmeinnicht");
    // KLocalizedQmlContext (KF ≥ 6.8) — Nachfolger des deprecated KLocalizedContext.
    auto *context = new KLocalizedQmlContext(&engine);
    context->setTranslationDomain(QStringLiteral("vergissmeinnicht"));
    engine.rootContext()->setContextObject(context);
}

void vmnSetUiLanguage(const QString &language)
{
    if (!language.isEmpty()) {
        KLocalizedString::setLanguages({language});
    }
}
