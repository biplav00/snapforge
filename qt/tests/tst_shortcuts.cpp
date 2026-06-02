// Pure-logic tests for the dynamic-hotkeys chord layer (shortcuts::).
//
// glyphs() and toCarbon() are pure string transforms with no FFI or display
// dependency, so this runs GUILESS in CI. chord() reads the shared config via
// the Rust FFI; we don't assert its stored value (that would depend on the
// user's config file) — only that it never returns empty for a known action,
// i.e. the built-in default always backstops a missing/cleared config.

#include <QtTest/QtTest>

#include "Shortcuts.h"

#ifdef Q_OS_MAC
// kVK_ANSI_* / cmdKey / shiftKey etc. — the same constants toCarbon() emits.
#include <Carbon/Carbon.h>
#endif

class TestShortcuts : public QObject {
    Q_OBJECT

private slots:
    // --- glyphs(): chord string -> macOS symbol form ---

    void glyphs_emptyChord_returnsEmpty();
    void glyphs_modifiersAndLetter_data();
    void glyphs_modifiersAndLetter();
    void glyphs_caseInsensitiveAndAliases();
    void glyphs_namedKeys();
    void glyphs_trimsWhitespaceAroundTokens();

#ifdef Q_OS_MAC
    // --- toCarbon(): chord string -> Carbon vk + modifier mask ---

    void toCarbon_nullOutParams_returnFalse();
    void toCarbon_unknownKey_returnsFalse();
    void toCarbon_modifierOnly_returnsFalse();
    void toCarbon_accumulatesModifiers();
    void toCarbon_letterAndNamedKeys();
#endif

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

#ifdef Q_OS_MAC
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
#endif

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
