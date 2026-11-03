// Pure-logic tests for the dynamic-hotkeys chord layer (shortcuts::).
//
// glyphs() and toCarbon() are pure string transforms with no display
// dependency, so this runs GUILESS in CI. The target links the in-memory
// SnapforgeClient fake, so chord() reads a test-controlled config — never the
// developer's real ~/.config/snapforge/config.json (hermetic).
//
// SNAPFORGE_HOTKEY_ANSI_ONLY=1 pins toCarbon to the US ANSI keycode table so
// the expectations hold whatever keyboard layout the machine has.

#include <QtTest/QtTest>

#include "Shortcuts.h"
#include "SnapforgeClientTesting.h"

#ifdef Q_OS_MACOS
// kVK_ANSI_* / cmdKey / shiftKey etc. — the same constants toCarbon() emits.
#include <Carbon/Carbon.h>
#endif

class TestShortcuts : public QObject {
    Q_OBJECT

private slots:
    void initTestCase() {
        qputenv("SNAPFORGE_HOTKEY_ANSI_ONLY", "1");
        QStandardPaths::setTestModeEnabled(true);
    }
    void init() { sf::test::reset(); }

    // --- glyphs(): chord string -> macOS symbol form ---

    void glyphs_emptyChord_returnsEmpty();
    void glyphs_modifiersAndLetter_data();
    void glyphs_modifiersAndLetter();
    void glyphs_caseInsensitiveAndAliases();
    void glyphs_namedKeys();
    void glyphs_trimsWhitespaceAroundTokens();

#ifdef Q_OS_MACOS
    // --- toCarbon(): chord string -> Carbon vk + modifier mask ---

    void toCarbon_nullOutParams_returnFalse();
    void toCarbon_unknownKey_returnsFalse();
    void toCarbon_modifierOnly_returnsFalse();
    void toCarbon_accumulatesModifiers();
    void toCarbon_letterAndNamedKeys();
    void toCarbon_fKeysArrowsNavigation();
    void toCarbon_plusKey();
    void toCarbon_deleteVsBackspace();
    void toCarbon_ctrlIsControlKey();
#endif

    // --- Plus token / Ctrl parsing / global-chord validation ---
    void chordTokens_plusSpellings();
    void toQtKey_ctrlIsMeta_cmdIsControl();
    void toQtKey_namedKeys();
    void glyphs_plusAndNavigation();
    void isValidGlobalChord_data();
    void isValidGlobalChord();
    void camelCaseAlias_compiles();

    // --- chord(): config value vs default ---
    void chord_readsConfigValue();

    // --- chord(): default backstop ---

    void chord_knownActions_neverEmpty();
    void chord_unknownAction_isEmpty();
};

void TestShortcuts::glyphs_emptyChord_returnsEmpty() {
    QCOMPARE(shortcuts::glyphs(QString()), QString());
    QCOMPARE(shortcuts::glyphs(QStringLiteral("")), QString());
}

void TestShortcuts::glyphs_modifiersAndLetter_data() {
    QTest::addColumn<QString>("chord");
    QTest::addColumn<QString>("expected");

    QTest::newRow("cmd+shift+s") << "Cmd+Shift+S" << QStringLiteral("⌘⇧S");
    QTest::newRow("cmd+shift+f") << "Cmd+Shift+F" << QStringLiteral("⌘⇧F");
    QTest::newRow("ctrl+alt+r")  << "Ctrl+Alt+R"  << QStringLiteral("⌃⌥R");
    QTest::newRow("opt+1")       << "Opt+1"       << QStringLiteral("⌥1");
}

void TestShortcuts::glyphs_modifiersAndLetter() {
    QFETCH(QString, chord);
    QFETCH(QString, expected);
    QCOMPARE(shortcuts::glyphs(chord), expected);
}

void TestShortcuts::glyphs_caseInsensitiveAndAliases() {
    // command/cmd, option/opt/alt, control/ctrl all map to the same glyph
    // regardless of case.
    const QString cmd = QStringLiteral("⌘");
    const QString opt = QStringLiteral("⌥");
    const QString ctl = QStringLiteral("⌃");
    QCOMPARE(shortcuts::glyphs(QStringLiteral("command+a")), cmd + QStringLiteral("A"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("CMD+a")), cmd + QStringLiteral("A"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("option+a")), opt + QStringLiteral("A"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("alt+a")), opt + QStringLiteral("A"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("control+a")), ctl + QStringLiteral("A"));
}

void TestShortcuts::glyphs_namedKeys() {
    QCOMPARE(shortcuts::glyphs(QStringLiteral("escape")), QStringLiteral("⎋"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("return")), QStringLiteral("↩"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("enter")), QStringLiteral("↩"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("space")), QStringLiteral("␣"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("tab")), QStringLiteral("⇥"));
}

void TestShortcuts::glyphs_trimsWhitespaceAroundTokens() {
    // The badge editor may emit "Cmd + Shift + S" with spaces.
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd + Shift + S")),
             QStringLiteral("⌘⇧S"));
}

#ifdef Q_OS_MACOS
void TestShortcuts::toCarbon_nullOutParams_returnFalse() {
    uint32_t vk = 0, mods = 0;
    QVERIFY(!shortcuts::toCarbon(QStringLiteral("Cmd+S"), nullptr, &mods));
    QVERIFY(!shortcuts::toCarbon(QStringLiteral("Cmd+S"), &vk, nullptr));
}

void TestShortcuts::toCarbon_unknownKey_returnsFalse() {
    // A token with no known virtual keycode must be rejected so the caller
    // skips RegisterEventHotKey instead of binding garbage.
    uint32_t vk = 0xDEAD, mods = 0;
    QVERIFY(!shortcuts::toCarbon(QStringLiteral("Cmd+Ω"), &vk, &mods));
}

void TestShortcuts::toCarbon_modifierOnly_returnsFalse() {
    // No non-modifier key token -> no key to bind.
    uint32_t vk = 0, mods = 0;
    QVERIFY(!shortcuts::toCarbon(QStringLiteral("Cmd+Shift"), &vk, &mods));
}

void TestShortcuts::toCarbon_accumulatesModifiers() {
    uint32_t vk = 0, mods = 0;
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Shift+Alt+Ctrl+S"), &vk, &mods));
    // All four modifier bits must be set; exact bit values are Carbon's.
    QVERIFY((mods & cmdKey) != 0);
    QVERIFY((mods & shiftKey) != 0);
    QVERIFY((mods & optionKey) != 0);
    QVERIFY((mods & controlKey) != 0);
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ANSI_S));
}

void TestShortcuts::toCarbon_letterAndNamedKeys() {
    uint32_t vk = 0, mods = 0;
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+R"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ANSI_R));
    QCOMPARE(mods, static_cast<uint32_t>(cmdKey));

    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Space"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_Space));

    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Return"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_Return));
}

void TestShortcuts::toCarbon_fKeysArrowsNavigation() {
    uint32_t vk = 0, mods = 0;
    struct { const char *chord; int vk; } cases[] = {
        {"Cmd+F1", kVK_F1}, {"Cmd+F5", kVK_F5}, {"Cmd+F12", kVK_F12},
        {"Cmd+Left", kVK_LeftArrow}, {"Cmd+Right", kVK_RightArrow},
        {"Cmd+Up", kVK_UpArrow}, {"Cmd+Down", kVK_DownArrow},
        {"Cmd+Home", kVK_Home}, {"Cmd+End", kVK_End},
        {"Cmd+PgUp", kVK_PageUp}, {"Cmd+PageDown", kVK_PageDown},
    };
    for (const auto &c : cases) {
        QVERIFY2(shortcuts::toCarbon(QString::fromLatin1(c.chord), &vk, &mods), c.chord);
        QCOMPARE(vk, static_cast<uint32_t>(c.vk));
    }
}

void TestShortcuts::toCarbon_plusKey() {
    // "Plus" (what the Preferences recorder emits) and a trailing "++"
    // (QKeySequence spelling) both bind the =/+ key with only the stated mods.
    uint32_t vk = 0, mods = 0;
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Plus"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ANSI_Equal));
    QCOMPARE(mods, static_cast<uint32_t>(cmdKey));
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd++"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ANSI_Equal));
    QCOMPARE(mods, static_cast<uint32_t>(cmdKey));
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Shift++"), &vk, &mods));
    QCOMPARE(mods, static_cast<uint32_t>(cmdKey | shiftKey));
}

void TestShortcuts::toCarbon_deleteVsBackspace() {
    uint32_t vk = 0, mods = 0;
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Backspace"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_Delete));         // ⌫
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Cmd+Delete"), &vk, &mods));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ForwardDelete));  // ⌦
}

void TestShortcuts::toCarbon_ctrlIsControlKey() {
    uint32_t vk = 0, mods = 0;
    QVERIFY(shortcuts::toCarbon(QStringLiteral("Ctrl+Alt+R"), &vk, &mods));
    QCOMPARE(mods, static_cast<uint32_t>(controlKey | optionKey));
    QCOMPARE(vk, static_cast<uint32_t>(kVK_ANSI_R));
}
#endif

void TestShortcuts::chordTokens_plusSpellings() {
    QCOMPARE(shortcuts::chordTokens(QStringLiteral("Cmd++")),
             QStringList({QStringLiteral("Cmd"), QStringLiteral("+")}));
    QCOMPARE(shortcuts::chordTokens(QStringLiteral("Cmd + Shift + +")),
             QStringList({QStringLiteral("Cmd"), QStringLiteral("Shift"), QStringLiteral("+")}));
    QCOMPARE(shortcuts::chordTokens(QStringLiteral("+")), QStringList({QStringLiteral("+")}));
    QCOMPARE(shortcuts::chordTokens(QStringLiteral("Cmd+Plus")),
             QStringList({QStringLiteral("Cmd"), QStringLiteral("Plus")}));
}

void TestShortcuts::toQtKey_ctrlIsMeta_cmdIsControl() {
    int key = 0;
    Qt::KeyboardModifiers mods;
    QVERIFY(shortcuts::toQtKey(QStringLiteral("Ctrl+S"), &key, &mods));
    QCOMPARE(mods, Qt::KeyboardModifiers(Qt::MetaModifier));
    QCOMPARE(key, int(Qt::Key_S));
    QVERIFY(shortcuts::toQtKey(QStringLiteral("Cmd+Ctrl+S"), &key, &mods));
    QCOMPARE(mods, Qt::KeyboardModifiers(Qt::ControlModifier | Qt::MetaModifier));
}

void TestShortcuts::toQtKey_namedKeys() {
    int key = 0;
    Qt::KeyboardModifiers mods;
    QVERIFY(shortcuts::toQtKey(QStringLiteral("Cmd+Plus"), &key, &mods));
    QCOMPARE(key, int(Qt::Key_Plus));
    QVERIFY(shortcuts::toQtKey(QStringLiteral("Cmd++"), &key, &mods));
    QCOMPARE(key, int(Qt::Key_Plus));
    QVERIFY(shortcuts::toQtKey(QStringLiteral("F5"), &key, &mods));
    QCOMPARE(key, int(Qt::Key_F5));
    QVERIFY(shortcuts::toQtKey(QStringLiteral("Alt+Left"), &key, &mods));
    QCOMPARE(key, int(Qt::Key_Left));
    QVERIFY(shortcuts::toQtKey(QStringLiteral("PageDown"), &key, &mods));
    QCOMPARE(key, int(Qt::Key_PageDown));
}

void TestShortcuts::glyphs_plusAndNavigation() {
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd+Plus")), QStringLiteral("⌘+"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd++")), QStringLiteral("⌘+"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd+Left")), QStringLiteral("⌘←"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd+F5")), QStringLiteral("⌘F5"));
    QCOMPARE(shortcuts::glyphs(QStringLiteral("Cmd+Delete")), QStringLiteral("⌘⌦"));
}

void TestShortcuts::isValidGlobalChord_data() {
    QTest::addColumn<QString>("chord");
    QTest::addColumn<bool>("valid");
    QTest::newRow("cmd+shift+s") << "Cmd+Shift+S" << true;
    QTest::newRow("ctrl+s")      << "Ctrl+S"      << true;
    QTest::newRow("alt+f5")      << "Alt+F5"      << true;
    QTest::newRow("cmd+plus")    << "Cmd+Plus"    << true;
    QTest::newRow("cmd++")       << "Cmd++"       << true;
    QTest::newRow("bare letter") << "S"           << false;
    QTest::newRow("shift only")  << "Shift+S"     << false;
    QTest::newRow("bare f5")     << "F5"          << false;
    QTest::newRow("mods only")   << "Cmd+Shift"   << false;
    QTest::newRow("unknown key") << "Cmd+Ω"       << false;
    QTest::newRow("empty")       << ""            << false;
}

void TestShortcuts::isValidGlobalChord() {
    QFETCH(QString, chord);
    QFETCH(bool, valid);
    QCOMPARE(shortcuts::isValidGlobalChord(chord), valid);
}

void TestShortcuts::camelCaseAlias_compiles() {
    // The UI calls it as Shortcuts::isValidGlobalChord(...).
    QVERIFY(Shortcuts::isValidGlobalChord(QStringLiteral("Cmd+K")));
}

void TestShortcuts::chord_readsConfigValue() {
    sf::test::state().configJson =
        QByteArrayLiteral(R"({"hotkeys":{"global":{"screenshot":"Ctrl+Alt+P"}}})");
    QCOMPARE(shortcuts::chord(QStringLiteral("screenshot")), QStringLiteral("Ctrl+Alt+P"));
    // Absent key -> built-in default.
    QCOMPARE(shortcuts::chord(QStringLiteral("record")), QStringLiteral("Cmd+Shift+R"));
}

void TestShortcuts::chord_knownActions_neverEmpty() {
    // Every canonical action resolves to a non-empty chord: a present config
    // value, or — when absent/cleared — the built-in default.
    for (const auto &a : shortcuts::kGlobalActions) {
        const QString key = QString::fromLatin1(a.actionKey);
        const QString c = shortcuts::chord(key);
        QVERIFY2(!c.isEmpty(),
                 qPrintable(QStringLiteral("chord(%1) was empty").arg(key)));
    }
}

void TestShortcuts::chord_unknownAction_isEmpty() {
    QCOMPARE(shortcuts::chord(QStringLiteral("no_such_action")), QString());
}

QTEST_GUILESS_MAIN(TestShortcuts)
#include "tst_shortcuts.moc"
