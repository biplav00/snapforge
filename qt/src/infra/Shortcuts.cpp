#include "Shortcuts.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QHash>

#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>
#endif

namespace shortcuts {

static QString defaultChordFor(const QString &actionKey) {
    for (const auto &a : kGlobalActions) {
        if (actionKey == QLatin1String(a.actionKey))
            return QString::fromLatin1(a.defaultChord);
    }
    return {};
}

QString chord(const QString &actionKey) {
    QString fallback = defaultChordFor(actionKey);

    char *raw = snapforge_config_load();
    if (!raw)
        return fallback;
    QByteArray bytes(raw);
    snapforge_free_string(raw);

    QJsonParseError err{};
    QJsonDocument doc = QJsonDocument::fromJson(bytes, &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject())
        return fallback;

    QJsonObject global = doc.object()
                             .value(QStringLiteral("hotkeys")).toObject()
                             .value(QStringLiteral("global")).toObject();
    QString stored = global.value(actionKey).toString();
    return stored.isEmpty() ? fallback : stored;
}

QString glyphs(const QString &chord) {
    if (chord.isEmpty())
        return {};

    QString out;
    const QStringList tokens = chord.split(QLatin1Char('+'), Qt::SkipEmptyParts);
    for (QString tok : tokens) {
        tok = tok.trimmed();
        if (tok.compare(QLatin1String("cmd"), Qt::CaseInsensitive) == 0 ||
            tok.compare(QLatin1String("command"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⌘"); // ⌘
        else if (tok.compare(QLatin1String("shift"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⇧"); // ⇧
        else if (tok.compare(QLatin1String("alt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("opt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("option"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⌥"); // ⌥
        else if (tok.compare(QLatin1String("ctrl"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("control"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⌃"); // ⌃
        else if (tok.compare(QLatin1String("escape"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⎋"); // ⎋
        else if (tok.compare(QLatin1String("enter"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("return"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("↩"); // ↩
        else if (tok.compare(QLatin1String("space"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("␣"); // ␣
        else if (tok.compare(QLatin1String("tab"), Qt::CaseInsensitive) == 0)
            out += QStringLiteral("⇥"); // ⇥
        else
            out += tok.toUpper(); // letters, digits, comma, etc.
    }
    return out;
}

#ifdef Q_OS_MAC
// ANSI virtual keycodes (kVK_ANSI_*) are layout-position constants and stable.
static uint32_t virtualKeyFor(const QString &keyTok) {
    static const QHash<QString, uint32_t> kMap = {
        {QStringLiteral("A"), kVK_ANSI_A}, {QStringLiteral("B"), kVK_ANSI_B},
        {QStringLiteral("C"), kVK_ANSI_C}, {QStringLiteral("D"), kVK_ANSI_D},
        {QStringLiteral("E"), kVK_ANSI_E}, {QStringLiteral("F"), kVK_ANSI_F},
        {QStringLiteral("G"), kVK_ANSI_G}, {QStringLiteral("H"), kVK_ANSI_H},
        {QStringLiteral("I"), kVK_ANSI_I}, {QStringLiteral("J"), kVK_ANSI_J},
        {QStringLiteral("K"), kVK_ANSI_K}, {QStringLiteral("L"), kVK_ANSI_L},
        {QStringLiteral("M"), kVK_ANSI_M}, {QStringLiteral("N"), kVK_ANSI_N},
        {QStringLiteral("O"), kVK_ANSI_O}, {QStringLiteral("P"), kVK_ANSI_P},
        {QStringLiteral("Q"), kVK_ANSI_Q}, {QStringLiteral("R"), kVK_ANSI_R},
        {QStringLiteral("S"), kVK_ANSI_S}, {QStringLiteral("T"), kVK_ANSI_T},
        {QStringLiteral("U"), kVK_ANSI_U}, {QStringLiteral("V"), kVK_ANSI_V},
        {QStringLiteral("W"), kVK_ANSI_W}, {QStringLiteral("X"), kVK_ANSI_X},
        {QStringLiteral("Y"), kVK_ANSI_Y}, {QStringLiteral("Z"), kVK_ANSI_Z},
        {QStringLiteral("0"), kVK_ANSI_0}, {QStringLiteral("1"), kVK_ANSI_1},
        {QStringLiteral("2"), kVK_ANSI_2}, {QStringLiteral("3"), kVK_ANSI_3},
        {QStringLiteral("4"), kVK_ANSI_4}, {QStringLiteral("5"), kVK_ANSI_5},
        {QStringLiteral("6"), kVK_ANSI_6}, {QStringLiteral("7"), kVK_ANSI_7},
        {QStringLiteral("8"), kVK_ANSI_8}, {QStringLiteral("9"), kVK_ANSI_9},
        {QStringLiteral(","), kVK_ANSI_Comma}, {QStringLiteral("."), kVK_ANSI_Period},
        {QStringLiteral("/"), kVK_ANSI_Slash}, {QStringLiteral(";"), kVK_ANSI_Semicolon},
        {QStringLiteral("'"), kVK_ANSI_Quote}, {QStringLiteral("["), kVK_ANSI_LeftBracket},
        {QStringLiteral("]"), kVK_ANSI_RightBracket}, {QStringLiteral("-"), kVK_ANSI_Minus},
        {QStringLiteral("="), kVK_ANSI_Equal}, {QStringLiteral("`"), kVK_ANSI_Grave},
    };
    static const QHash<QString, uint32_t> kNamed = {
        {QStringLiteral("space"), kVK_Space}, {QStringLiteral("return"), kVK_Return},
        {QStringLiteral("enter"), kVK_Return}, {QStringLiteral("tab"), kVK_Tab},
        {QStringLiteral("escape"), kVK_Escape}, {QStringLiteral("delete"), kVK_Delete},
        {QStringLiteral("backspace"), kVK_Delete},
    };
    auto namedIt = kNamed.constFind(keyTok.toLower());
    if (namedIt != kNamed.constEnd())
        return namedIt.value();
    auto it = kMap.constFind(keyTok.toUpper());
    return it == kMap.constEnd() ? 0xFFFFFFFFu : it.value();
}

bool toCarbon(const QString &chord, uint32_t *vk, uint32_t *mods) {
    if (!vk || !mods)
        return false;
    *mods = 0;
    uint32_t key = 0xFFFFFFFFu;

    const QStringList tokens = chord.split(QLatin1Char('+'), Qt::SkipEmptyParts);
    for (QString tok : tokens) {
        tok = tok.trimmed();
        if (tok.compare(QLatin1String("cmd"), Qt::CaseInsensitive) == 0 ||
            tok.compare(QLatin1String("command"), Qt::CaseInsensitive) == 0)
            *mods |= cmdKey;
        else if (tok.compare(QLatin1String("shift"), Qt::CaseInsensitive) == 0)
            *mods |= shiftKey;
        else if (tok.compare(QLatin1String("alt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("opt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("option"), Qt::CaseInsensitive) == 0)
            *mods |= optionKey;
        else if (tok.compare(QLatin1String("ctrl"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("control"), Qt::CaseInsensitive) == 0)
            *mods |= controlKey;
        else
            key = virtualKeyFor(tok); // the (single) non-modifier token
    }

    if (key == 0xFFFFFFFFu)
        return false;
    *vk = key;
    return true;
}
#endif

} // namespace shortcuts
