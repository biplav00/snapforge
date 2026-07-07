#include "Shortcuts.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QHash>
#include <QKeySequence>

#include "SnapforgeClient.h"

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

Snapshot::Snapshot() {
    QByteArray bytes = sf::configLoadJson();
    if (bytes.isEmpty())
        return;

    QJsonParseError err{};
    QJsonDocument doc = QJsonDocument::fromJson(bytes, &err);
    if (err.error != QJsonParseError::NoError || !doc.isObject())
        return;

    m_hotkeys = doc.object().value(QStringLiteral("hotkeys")).toObject();
}

QString Snapshot::chord(const QString &actionKey) const {
    const QString stored = m_hotkeys.value(QStringLiteral("global")).toObject()
                               .value(actionKey).toString();
    return stored.isEmpty() ? defaultChordFor(actionKey) : stored;
}

QString Snapshot::chord(const QString &section, const QString &actionId,
                        const QString &defaultChord) const {
    const QString stored = m_hotkeys.value(section).toObject()
                               .value(actionId).toString();
    return stored.isEmpty() ? defaultChord : stored;
}

QString chord(const QString &actionKey) {
    return Snapshot().chord(actionKey);
}

QString chord(const QString &section, const QString &actionId,
              const QString &defaultChord) {
    return Snapshot().chord(section, actionId, defaultChord);
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

bool toQtKey(const QString &chord, int *key, Qt::KeyboardModifiers *mods) {
    if (!key || !mods)
        return false;
    *mods = Qt::NoModifier;
    int k = 0;

    const QStringList tokens = chord.split(QLatin1Char('+'), Qt::SkipEmptyParts);
    for (QString tok : tokens) {
        tok = tok.trimmed();
        if (tok.compare(QLatin1String("cmd"), Qt::CaseInsensitive) == 0 ||
            tok.compare(QLatin1String("command"), Qt::CaseInsensitive) == 0)
            *mods |= Qt::ControlModifier; // ⌘ arrives as ControlModifier on macOS
        else if (tok.compare(QLatin1String("shift"), Qt::CaseInsensitive) == 0)
            *mods |= Qt::ShiftModifier;
        else if (tok.compare(QLatin1String("alt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("opt"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("option"), Qt::CaseInsensitive) == 0)
            *mods |= Qt::AltModifier;
        else if (tok.compare(QLatin1String("ctrl"), Qt::CaseInsensitive) == 0 ||
                 tok.compare(QLatin1String("control"), Qt::CaseInsensitive) == 0)
            *mods |= Qt::MetaModifier; // ⌃ arrives as MetaModifier on macOS
        else {
            // The (single) non-modifier token. Named keys match what
            // PreferencesWindow::formatKeyToShortcut emits.
            const QString lower = tok.toLower();
            if (lower == QLatin1String("escape"))
                k = Qt::Key_Escape;
            else if (lower == QLatin1String("enter") || lower == QLatin1String("return"))
                k = Qt::Key_Return;
            else if (lower == QLatin1String("space"))
                k = Qt::Key_Space;
            else if (lower == QLatin1String("tab"))
                k = Qt::Key_Tab;
            else if (lower == QLatin1String("backspace"))
                k = Qt::Key_Backspace;
            else if (lower == QLatin1String("delete"))
                k = Qt::Key_Delete;
            else if (tok.size() == 1)
                k = tok.toUpper().at(0).unicode(); // letters, digits, punctuation
            else {
                // F-keys, arrows, Home, ... — formatKeyToShortcut emits these
                // via QKeySequence, so parse them back the same way.
                const QKeySequence seq = QKeySequence::fromString(tok);
                if (seq.count() == 1)
                    k = seq[0].key();
            }
        }
    }

    if (k == 0)
        return false;
    *key = k;
    return true;
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
