// Integration-ish smoke test.
//
// The most integration-y path we can drive WITHOUT a real capture session or a
// display is: load the shared config through the FFI, parse it the same way
// the app does, feed the hotkey chords through the chord layer, and confirm
// RecordingManager wires that config into a consistent idle state. This
// exercises the FFI <-> JSON <-> Qt boundary end to end.
//
// A second test attempts a real headless capture probe but is GATED on
// SNAPFORGE_REQUIRE_DISPLAY, mirroring crates/snapforge-app/tests/integration.rs:
// unset (the CI default) -> QSKIP; set -> the start must actually succeed.
//
// NOTE: this test is deliberately READ-ONLY against the user's config. It never
// calls snapforge_config_save, because AppConfig::save() writes the real
// ~/.config/snapforge/config.json — a test must not clobber the user's settings.

#include <QtTest/QtTest>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QJsonDocument>
#include <QJsonObject>

#include "RecordingManager.h"
#include "Shortcuts.h"

#include "snapforge_ffi.h"

namespace {
bool requireDisplay() {
    const QByteArray v = qgetenv("SNAPFORGE_REQUIRE_DISPLAY");
    return v == "1" || v.compare("true", Qt::CaseInsensitive) == 0;
}
} // namespace

class TestSmoke : public QObject {
    Q_OBJECT

private slots:
    void configLoad_isWellFormedJson();
    void configToHotkeys_roundTrip();
    void managerReflectsLoadedConfig();
    void headlessCaptureProbe_gatedOnDisplay();
};

void TestSmoke::configLoad_isWellFormedJson() {
    // config_load returns NULL only on a hard failure; defaults still parse to
    // a JSON object. Either NULL (treated as "use defaults") or valid object.
    char *raw = snapforge_config_load();
    if (!raw) {
        QSKIP("snapforge_config_load returned NULL; nothing to assert");
    }
    const QByteArray bytes(raw);
    snapforge_free_string(raw);

    QJsonParseError err{};
    const QJsonDocument doc = QJsonDocument::fromJson(bytes, &err);
    QCOMPARE(err.error, QJsonParseError::NoError);
    QVERIFY(doc.isObject());
}

void TestSmoke::configToHotkeys_roundTrip() {
    // Drive each canonical action through chord() (config-backed) and then
    // through the display + Carbon-parse transforms. Every default chord must
    // render to a non-empty glyph string and parse to a valid Carbon binding.
    for (const auto &a : shortcuts::kGlobalActions) {
        const QString key = QString::fromLatin1(a.actionKey);
        const QString chord = shortcuts::chord(key);
        QVERIFY2(!chord.isEmpty(), qPrintable(key));

        QVERIFY2(!shortcuts::glyphs(chord).isEmpty(), qPrintable(chord));

#ifdef Q_OS_MAC
        uint32_t vk = 0, mods = 0;
        QVERIFY2(shortcuts::toCarbon(chord, &vk, &mods),
                 qPrintable(QStringLiteral("toCarbon failed for %1=%2")
                                .arg(key, chord)));
        // Each default uses Cmd; the resolved chord must keep at least that.
        QVERIFY(mods != 0);
#endif
    }
}

void TestSmoke::managerReflectsLoadedConfig() {
    // Construct the manager, pull prefs from config, and assert it sits in a
    // coherent idle state — the realistic post-startup snapshot before the
    // user triggers anything.
    RecordingManager mgr;
    mgr.reloadPrefs();
    QVERIFY(!mgr.isRecording());
    QVERIFY(!mgr.isPaused());
    QCOMPARE(mgr.elapsedSeconds(), 0);
    QVERIFY(mgr.outputPath().isEmpty());
}

void TestSmoke::headlessCaptureProbe_gatedOnDisplay() {
    if (!requireDisplay()) {
        QSKIP("SNAPFORGE_REQUIRE_DISPLAY unset; skipping real-capture smoke "
              "(matches Rust integration-test gating)");
    }

    // A runner that claims a display MUST be able to actually start a capture.
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());

    QVERIFY2(startedSpy.count() == 1 && errorSpy.count() == 0,
             "SNAPFORGE_REQUIRE_DISPLAY=1 but recording failed to start");
    QVERIFY(mgr.isRecording());
    mgr.stopRecording();
}

QTEST_GUILESS_MAIN(TestSmoke)
#include "tst_smoke.moc"
