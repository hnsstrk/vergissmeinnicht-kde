#include "klocalized.h"

#include <KLocalizedContext>
#include <KLocalizedString>
#include <QQmlContext>
#include <QQmlEngine>

void vmnInstallKLocalizedContext(QQmlEngine &engine)
{
    KLocalizedString::setApplicationDomain("vergissmeinnicht");
    auto *context = new KLocalizedContext(&engine);
    context->setTranslationDomain(QStringLiteral("vergissmeinnicht"));
    engine.rootContext()->setContextObject(context);
}

void vmnSetUiLanguage(const QString &language)
{
    if (!language.isEmpty()) {
        KLocalizedString::setLanguages({language});
    }
}
