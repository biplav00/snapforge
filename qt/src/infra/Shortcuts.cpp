#include "Shortcuts.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QHash>
#include <QKeySequence>

#include "SnapforgeClient.h"

#ifdef Q_OS_MACOS
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

namespace {

enum ModBit : unsigned { kModCmd = 1, kModShift = 2, kModAlt = 4, kModCtrl = 8 };

// Modifier bit for a token, or 0 if it's the key token.
unsigned modBit(const QString &tok) {
    const QString t = tok.toLower();
    if (t == QLatin1String("cmd") || t == QLatin1String("command")) return kModCmd;
    if (t == QLatin1String("shift")) return kModShift;
    if (t == QLatin1String("alt") || t == QLatin1String("opt") ||
        t == QLatin1String("option"))
        return kModAlt;
    if (t == QLatin1String("ctrl") || t == QLatin1String("control")) return kModCtrl;
    return 0;
}

} // namespace

// Split a chord into tokens. '+' is the separator, so a literal plus key is
// spelled either "Plus" ("Cmd+Plus", what the Preferences recorder emits) or
// as a trailing "++" ("Cmd++", what QKeySequence::toString produces). A bare
// "+" chord is the plus key on its own. Whitespace is insignificant.
QStringList chordTokens(const QString &chord) {
    QString c = chord;
    c.remove(QLatin1Char(' '));
    if (c == QLatin1String("+"))
        return {QStringLiteral("+")};
    bool plusKey = false;
    if (c.endsWith(QLatin1String("++"))) {
        plusKey = true;
        c.chop(2);
    }
    QStringList out = c.split(QLatin1Char('+'), Qt::SkipEmptyParts);
    if (plusKey)
        out << QStringLiteral("+");
    return out;
}

QString glyphs(const QString &chord) {
    if (chord.isEmpty())
        return {};

    static const QHash<QString, QString> kNamedGlyphs = {
        {QStringLiteral("escape"), QStringLiteral("⎋")},
        {QStringLiteral("esc"), QStringLiteral("⎋")},
        {QStringLiteral("enter"), QStringLiteral("↩")},
        {QStringLiteral("return"), QStringLiteral("↩")},
        {QStringLiteral("space"), QStringLiteral("␣")},
        {QStringLiteral("tab"), QStringLiteral("⇥")},
        {QStringLiteral("backspace"), QStringLiteral("⌫")},
        {QStringLiteral("delete"), QStringLiteral("⌦")},
        {QStringLiteral("del"), QStringLiteral("⌦")},
        {QStringLiteral("left"), QStringLiteral("←")},
        {QStringLiteral("right"), QStringLiteral("→")},
        {QStringLiteral("up"), QStringLiteral("↑")},
        {QStringLiteral("down"), QStringLiteral("↓")},
        {QStringLiteral("home"), QStringLiteral("↖")},
        {QStringLiteral("end"), QStringLiteral("↘")},
        {QStringLiteral("pageup"), QStringLiteral("⇞")},
        {QStringLiteral("pgup"), QStringLiteral("⇞")},
        {QStringLiteral("pagedown"), QStringLiteral("⇟")},
        {QStringLiteral("pgdown"), QStringLiteral("⇟")},
        {QStringLiteral("plus"), QStringLiteral("+")},
    };

    QString out;
    for (const QString &tok : chordTokens(chord)) {
        switch (modBit(tok)) {
        case kModCmd:   out += QStringLiteral("⌘"); continue;
        case kModShift: out += QStringLiteral("⇧"); continue;
        case kModAlt:   out += QStringLiteral("⌥"); continue;
        case kModCtrl:  out += QStringLiteral("⌃"); continue;
        default: break;
        }
        auto it = kNamedGlyphs.constFind(tok.toLower());
        out += it != kNamedGlyphs.constEnd() ? it.value()
                                             : tok.toUpper(); // letters, digits, F-keys, punctuation
    }
    return out;
}

bool toQtKey(const QString &chord, int *key, Qt::KeyboardModifiers *mods) {
    if (!key || !mods)
        return false;
    *mods = Qt::NoModifier;
    int k = 0;

    static const QHash<QString, int> kNamed = {
        {QStringLiteral("escape"), Qt::Key_Escape},
        {QStringLiteral("esc"), Qt::Key_Escape},
        {QStringLiteral("enter"), Qt::Key_Return},
        {QStringLiteral("return"), Qt::Key_Return},
        {QStringLiteral("space"), Qt::Key_Space},
        {QStringLiteral("tab"), Qt::Key_Tab},
        {QStringLiteral("backspace"), Qt::Key_Backspace},
        {QStringLiteral("delete"), Qt::Key_Delete},
        {QStringLiteral("del"), Qt::Key_Delete},
        {QStringLiteral("left"), Qt::Key_Left},
        {QStringLiteral("right"), Qt::Key_Right},
        {QStringLiteral("up"), Qt::Key_Up},
        {QStringLiteral("down"), Qt::Key_Down},
        {QStringLiteral("home"), Qt::Key_Home},
        {QStringLiteral("end"), Qt::Key_End},
        {QStringLiteral("pageup"), Qt::Key_PageUp},
        {QStringLiteral("pgup"), Qt::Key_PageUp},
        {QStringLiteral("pagedown"), Qt::Key_PageDown},
        {QStringLiteral("pgdown"), Qt::Key_PageDown},
        {QStringLiteral("plus"), Qt::Key_Plus},
    };

    for (const QString &tok : chordTokens(chord)) {
        switch (modBit(tok)) {
        // Qt swaps Command/Control on macOS: ⌘ arrives as ControlModifier,
        // ⌃ as MetaModifier.
        case kModCmd:   *mods |= Qt::ControlModifier; continue;
        case kModShift: *mods |= Qt::ShiftModifier;   continue;
        case kModAlt:   *mods |= Qt::AltModifier;     continue;
        case kModCtrl:  *mods |= Qt::MetaModifier;    continue;
        default: break;
        }
        // The (single) non-modifier token. Named keys match what
        // PreferencesWindow::formatKeyToShortcut emits.
        auto it = kNamed.constFind(tok.toLower());
        if (it != kNamed.constEnd())
            k = it.value();
        else if (tok.size() == 1)
            k = tok.toUpper().at(0).unicode(); // letters, digits, punctuation
        else {
            // F-keys etc. — formatKeyToShortcut emits these via QKeySequence,
            // so parse them back the same way.
            const QKeySequence seq = QKeySequence::fromString(tok);
            if (seq.count() == 1)
                k = seq[0].key();
        }
    }

    if (k == 0)
        return false;
    *key = k;
    return true;
}

bool isValidGlobalChord(const QString &chord) {
    // A global hotkey without Cmd/Ctrl/Alt (bare "A", or "Shift+A") would
    // swallow ordinary typing system-wide, so it is never valid.
    unsigned mods = 0;
    for (const QString &tok : chordTokens(chord))
        mods |= modBit(tok);
    if ((mods & (kModCmd | kModCtrl | kModAlt)) == 0)
        return false;
#ifdef Q_OS_MACOS
    uint32_t vk = 0, cmods = 0;
    return toCarbon(chord, &vk, &cmods);
#else
    int key = 0;
    Qt::KeyboardModifiers qmods;
    return toQtKey(chord, &key, &qmods);
#endif
}

#ifdef Q_OS_MACOS
namespace {

constexpr uint32_t kNoKey = 0xFFFFFFFFu;

// ANSI virtual keycodes (kVK_ANSI_*) are physical-position constants: they
// only name the right key on a US layout. Used as the fallback when the
// current layout can't be queried (or doesn't produce the character).
const QHash<QChar, uint32_t> &ansiTable() {
    static const QHash<QChar, uint32_t> kMap = {
        {QChar('A'), kVK_ANSI_A}, {QChar('B'), kVK_ANSI_B}, {QChar('C'), kVK_ANSI_C},
        {QChar('D'), kVK_ANSI_D}, {QChar('E'), kVK_ANSI_E}, {QChar('F'), kVK_ANSI_F},
        {QChar('G'), kVK_ANSI_G}, {QChar('H'), kVK_ANSI_H}, {QChar('I'), kVK_ANSI_I},
        {QChar('J'), kVK_ANSI_J}, {QChar('K'), kVK_ANSI_K}, {QChar('L'), kVK_ANSI_L},
        {QChar('M'), kVK_ANSI_M}, {QChar('N'), kVK_ANSI_N}, {QChar('O'), kVK_ANSI_O},
        {QChar('P'), kVK_ANSI_P}, {QChar('Q'), kVK_ANSI_Q}, {QChar('R'), kVK_ANSI_R},
        {QChar('S'), kVK_ANSI_S}, {QChar('T'), kVK_ANSI_T}, {QChar('U'), kVK_ANSI_U},
        {QChar('V'), kVK_ANSI_V}, {QChar('W'), kVK_ANSI_W}, {QChar('X'), kVK_ANSI_X},
        {QChar('Y'), kVK_ANSI_Y}, {QChar('Z'), kVK_ANSI_Z},
        {QChar('0'), kVK_ANSI_0}, {QChar('1'), kVK_ANSI_1}, {QChar('2'), kVK_ANSI_2},
        {QChar('3'), kVK_ANSI_3}, {QChar('4'), kVK_ANSI_4}, {QChar('5'), kVK_ANSI_5},
        {QChar('6'), kVK_ANSI_6}, {QChar('7'), kVK_ANSI_7}, {QChar('8'), kVK_ANSI_8},
        {QChar('9'), kVK_ANSI_9},
        {QChar(','), kVK_ANSI_Comma}, {QChar('.'), kVK_ANSI_Period},
        {QChar('/'), kVK_ANSI_Slash}, {QChar(';'), kVK_ANSI_Semicolon},
        {QChar('\''), kVK_ANSI_Quote}, {QChar('['), kVK_ANSI_LeftBracket},
        {QChar(']'), kVK_ANSI_RightBracket}, {QChar('-'), kVK_ANSI_Minus},
        {QChar('='), kVK_ANSI_Equal}, {QChar('`'), kVK_ANSI_Grave},
        {QChar('\\'), kVK_ANSI_Backslash},
        // '+' is Shift+= on US; bind the physical key (Cmd+Plus == Cmd+= is
        // the universal "zoom in" convention) rather than adding Shift.
        {QChar('+'), kVK_ANSI_Equal},
    };
    return kMap;
}

// Character -> virtual keycode for the CURRENT keyboard layout, built by
// asking UCKeyTranslate what each main-block key types without modifiers.
// So on AZERTY "Cmd+A" binds the key labelled A (kVK_ANSI_Q's position), not
// the US-A position. Rebuilt on each call: it's only used when (re)registering
// a handful of hotkeys, and that way a layout switch is picked up on the next
// re-register. Empty if the layout data is unavailable.
//
// Test hook: SNAPFORGE_HOTKEY_ANSI_ONLY=1 forces the static ANSI table so
// unit tests are independent of the developer's keyboard layout.
QHash<QChar, uint32_t> layoutTable() {
    QHash<QChar, uint32_t> out;
    if (qEnvironmentVariableIntValue("SNAPFORGE_HOTKEY_ANSI_ONLY") == 1)
        return out;

    TISInputSourceRef src = TISCopyCurrentKeyboardLayoutInputSource();
    CFDataRef data = src ? static_cast<CFDataRef>(
        TISGetInputSourceProperty(src, kTISPropertyUnicodeKeyLayoutData)) : nullptr;
    if (!data) {
        // Non-Unicode input methods (some IMEs) have no layout data; the
        // ASCII-capable layout is what the hotkey keys map to in that case.
        if (src) CFRelease(src);
        src = TISCopyCurrentASCIICapableKeyboardLayoutInputSource();
        data = src ? static_cast<CFDataRef>(
            TISGetInputSourceProperty(src, kTISPropertyUnicodeKeyLayoutData)) : nullptr;
    }
    if (data) {
        const auto *layout =
            reinterpret_cast<const UCKeyboardLayout *>(CFDataGetBytePtr(data));
        // Main typing block only (kVK_ANSI_A=0x00 .. kVK_ANSI_Grave=0x32):
        // keypad keys (0x41+) also type digits and must not shadow the row.
        for (uint16_t code = 0; code <= kVK_ANSI_Grave; ++code) {
            UInt32 deadState = 0;
            UniChar chars[4];
            UniCharCount len = 0;
            const OSStatus st = UCKeyTranslate(
                layout, code, kUCKeyActionDisplay, 0, LMGetKbdType(),
                kUCKeyTranslateNoDeadKeysBit, &deadState, 4, &len, chars);
            if (st != noErr || len != 1) continue;
            const QChar ch = QChar(chars[0]).toUpper();
            if (ch.unicode() <= 0x20 || ch.unicode() == 0x7F) continue;
            if (!out.contains(ch)) out.insert(ch, code);
        }
    }
    if (src) CFRelease(src);
    return out;
}

uint32_t virtualKeyFor(const QString &keyTok, const QHash<QChar, uint32_t> &layout) {
    static const QHash<QString, uint32_t> kNamed = {
        {QStringLiteral("space"), kVK_Space},
        {QStringLiteral("return"), kVK_Return},
        {QStringLiteral("enter"), kVK_Return},
        {QStringLiteral("tab"), kVK_Tab},
        {QStringLiteral("escape"), kVK_Escape},
        {QStringLiteral("esc"), kVK_Escape},
        // Qt naming: Backspace is the ⌫ key (Carbon calls it kVK_Delete);
        // Delete is forward delete ⌦.
        {QStringLiteral("backspace"), kVK_Delete},
        {QStringLiteral("delete"), kVK_ForwardDelete},
        {QStringLiteral("del"), kVK_ForwardDelete},
        {QStringLiteral("left"), kVK_LeftArrow},
        {QStringLiteral("right"), kVK_RightArrow},
        {QStringLiteral("up"), kVK_UpArrow},
        {QStringLiteral("down"), kVK_DownArrow},
        {QStringLiteral("home"), kVK_Home},
        {QStringLiteral("end"), kVK_End},
        {QStringLiteral("pageup"), kVK_PageUp},
        {QStringLiteral("pgup"), kVK_PageUp},
        {QStringLiteral("pagedown"), kVK_PageDown},
        {QStringLiteral("pgdown"), kVK_PageDown},
        {QStringLiteral("f1"), kVK_F1},   {QStringLiteral("f2"), kVK_F2},
        {QStringLiteral("f3"), kVK_F3},   {QStringLiteral("f4"), kVK_F4},
        {QStringLiteral("f5"), kVK_F5},   {QStringLiteral("f6"), kVK_F6},
        {QStringLiteral("f7"), kVK_F7},   {QStringLiteral("f8"), kVK_F8},
        {QStringLiteral("f9"), kVK_F9},   {QStringLiteral("f10"), kVK_F10},
        {QStringLiteral("f11"), kVK_F11}, {QStringLiteral("f12"), kVK_F12},
    };
    const QString lower = keyTok.toLower();
    auto namedIt = kNamed.constFind(lower);
    if (namedIt != kNamed.constEnd())
        return namedIt.value();

    QChar ch;
    if (lower == QLatin1String("plus"))
        ch = QLatin1Char('+');
    else if (keyTok.size() == 1)
        ch = keyTok.at(0).toUpper();
    else
        return kNoKey;

    auto it = layout.constFind(ch);
    if (it != layout.constEnd())
        return it.value();
    auto ansi = ansiTable().constFind(ch);
    return ansi == ansiTable().constEnd() ? kNoKey : ansi.value();
}

} // namespace

bool toCarbon(const QString &chord, uint32_t *vk, uint32_t *mods) {
    if (!vk || !mods)
        return false;
    *mods = 0;
    uint32_t key = kNoKey;
    const QHash<QChar, uint32_t> layout = layoutTable();

    for (const QString &tok : chordTokens(chord)) {
        switch (modBit(tok)) {
        case kModCmd:   *mods |= cmdKey;     continue;
        case kModShift: *mods |= shiftKey;   continue;
        case kModAlt:   *mods |= optionKey;  continue;
        case kModCtrl:  *mods |= controlKey; continue;
        default: break;
        }
        key = virtualKeyFor(tok, layout); // the (single) non-modifier token
    }

    if (key == kNoKey)
        return false;
    *vk = key;
    return true;
}

int layoutUnshiftedQtKey(quint32 nativeVirtualKey) {
    if (nativeVirtualKey > kVK_ANSI_Grave)
        return 0;
    // Inverse lookup of the same table toCarbon() registers through, so a
    // recorded chord always binds the physical key the user pressed.
    const QHash<QChar, uint32_t> layout = layoutTable();
    for (auto it = layout.cbegin(); it != layout.cend(); ++it) {
        if (it.value() == nativeVirtualKey) {
            const QChar ch = it.key(); // already upper-cased
            // Qt::Key values for Latin-1 printable characters equal their
            // (upper-case) code point.
            return ch.unicode() < 0x100 ? int(ch.unicode()) : 0;
        }
    }
    return 0;
}
#endif

} // namespace shortcuts
