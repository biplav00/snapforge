// PreferencesWindow config round-trip through the SnapforgeClient SEAM (fake).
// Proves the migrated config load/save path: a known config returned by the
// fake lands in the widgets, and a save serializes back through the seam. No
// real FFI, no config file on disk. Runs offscreen (QT_QPA_PLATFORM set by the
// CMake test ENVIRONMENT) so it needs no window server.

#include <QtTest/QtTest>
#include <QSignalSpy>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSlider>
#include <QLineEdit>

#include "PreferencesWindow.h"
#include "SnapforgeClientTesting.h"

namespace {
QByteArray knownConfig() {
    return QByteArrayLiteral(R"({
        "save_directory": "/tmp/snaptest",
        "auto_copy_clipboard": true,
        "show_notification": true,
        "screenshot_format": "Jpg",
        "jpg_quality": 75,
        "remember_last_region": true,
        "last_region": {"x": 10, "y": 20, "w": 300, "h": 200},
        "filename_pattern": "shot-{date}",
        "recording": {"format": "Mp4", "fps": 30, "quality": "Medium"},
        "hotkeys": {}
    })");
}
} // namespace

class TestPreferences : public QObject {
    Q_OBJECT
private slots:
    void init() {
        sf::test::reset();
        sf::test::state().configJson = knownConfig();
    }

    void load_appliesConfigToWidgets() {
        // loadConfig() runs from showEvent(), so show the window (offscreen) to
        // trigger it; it reads sf::configLoadJson().
        PreferencesWindow win;
        win.show();
        QApplication::processEvents();
        QCOMPARE(win.saveDirectory(), QStringLiteral("/tmp/snaptest"));
        QCOMPARE(win.jpgQuality(), 75);
        QVERIFY(win.autoCopyEnabled());
    }

    // Regression: main.cpp reads the getters for every capture from app start,
    // possibly without the window ever being shown. They must reflect the
    // SAVED config right after construction.
    void freshWindow_notShown_returnsSavedConfig() {
        PreferencesWindow win; // never shown
        QVERIFY(!win.isVisible());
        QCOMPARE(win.saveDirectory(), QStringLiteral("/tmp/snaptest"));
        QCOMPARE(win.screenshotFormat(), 1); // Jpg
        QCOMPARE(win.jpgQuality(), 75);
        QCOMPARE(win.filenamePattern(), QStringLiteral("shot-{date}"));
        QVERIFY(win.autoCopyEnabled());
        QVERIFY(win.showNotifEnabled());
        QVERIFY(win.rememberRegionEnabled());
        QCOMPARE(win.lastRegion(), QRect(10, 20, 300, 200));
    }

    // Unsaved widget edits must not leak into behaviour.
    void unsavedEdits_doNotChangeGetters() {
        PreferencesWindow win;
        auto *slider = win.findChild<QSlider *>();
        QVERIFY(slider);
        slider->setValue(12);
        QCOMPARE(win.jpgQuality(), 75);
    }

    // setLastRegion must never write a partial config when the on-disk one
    // can't be read (that would wipe every other setting).
    void setLastRegion_unreadableConfig_keepsOtherFields() {
        PreferencesWindow win; // snapshot loaded from knownConfig()
        sf::test::state().configJson = QByteArray(); // disk read now fails
        sf::test::state().savedConfigJson.clear();
        win.setLastRegion(QRect(1, 2, 30, 40));
        const QJsonObject obj =
            QJsonDocument::fromJson(sf::test::state().savedConfigJson).object();
        QCOMPARE(obj.value(QStringLiteral("save_directory")).toString(),
                 QStringLiteral("/tmp/snaptest"));
        QCOMPARE(obj.value(QStringLiteral("last_region")).toObject()
                     .value(QStringLiteral("w")).toInt(), 30);
    }

    void setLastRegion_noSnapshot_skipsPersist() {
        sf::test::state().configJson = QByteArray();
        PreferencesWindow win; // load fails -> no snapshot
        sf::test::state().savedConfigJson.clear();
        win.setLastRegion(QRect(1, 2, 30, 40));
        QVERIFY(sf::test::state().savedConfigJson.isEmpty());
        QCOMPARE(win.lastRegion(), QRect(1, 2, 30, 40));
    }

    // Recorder mapping: Qt reports physical ⌃ as MetaModifier and ⌘ as
    // ControlModifier on macOS.
    void recorder_mapsCtrlMetaShiftAndPlus() {
        QCOMPARE(PreferencesWindow::formatKeyToShortcut(Qt::Key_A, Qt::MetaModifier),
                 QStringLiteral("Ctrl+A"));
        QCOMPARE(PreferencesWindow::formatKeyToShortcut(
                     Qt::Key_S, Qt::ControlModifier | Qt::ShiftModifier),
                 QStringLiteral("Cmd+Shift+S"));
        QCOMPARE(PreferencesWindow::formatKeyToShortcut(
                     Qt::Key_K, Qt::ControlModifier | Qt::MetaModifier | Qt::AltModifier),
                 QStringLiteral("Cmd+Ctrl+Alt+K"));
        // Shift+1 arrives as Key_Exclam; record the unshifted key.
        QCOMPARE(PreferencesWindow::formatKeyToShortcut(
                     Qt::Key_Exclam, Qt::ControlModifier | Qt::ShiftModifier),
                 QStringLiteral("Cmd+Shift+1"));
        QCOMPARE(PreferencesWindow::formatKeyToShortcut(Qt::Key_Plus, Qt::ControlModifier),
                 QStringLiteral("Cmd+Plus"));
        QVERIFY(PreferencesWindow::formatKeyToShortcut(Qt::Key_Shift, Qt::ShiftModifier).isEmpty());
    }

    void save_rejectsBadFilenamePattern() {
        PreferencesWindow win;
        auto edits = win.findChildren<QLineEdit *>();
        QLineEdit *pattern = nullptr;
        for (QLineEdit *e : edits)
            if (e->text() == QStringLiteral("shot-{date}")) pattern = e;
        QVERIFY(pattern);
        pattern->setText(QStringLiteral("../escape"));
        sf::test::state().savedConfigJson.clear();
        QSignalSpy savedSpy(&win, &PreferencesWindow::configSaved);
        // onSave shows a modal warning on failure; close it as soon as it opens.
        QTimer::singleShot(0, []() {
            if (QWidget *w = QApplication::activeModalWidget()) w->close();
        });
        QMetaObject::invokeMethod(&win, "onSave", Qt::DirectConnection);
        QCOMPARE(savedSpy.count(), 0);
        QVERIFY(sf::test::state().savedConfigJson.isEmpty());
    }

    void save_serializesBackThroughSeam() {
        PreferencesWindow win;
        win.show();
        QApplication::processEvents();
        QSignalSpy savedSpy(&win, &PreferencesWindow::configSaved);

        // onSave is a private slot but invokable by name via the meta-object.
        const bool invoked =
            QMetaObject::invokeMethod(&win, "onSave", Qt::DirectConnection);
        QVERIFY(invoked);

        QCOMPARE(savedSpy.count(), 1);
        // The fake captured exactly what the window serialized.
        const QByteArray saved = sf::test::state().savedConfigJson;
        QVERIFY(!saved.isEmpty());
        const QJsonObject obj = QJsonDocument::fromJson(saved).object();
        QVERIFY(!obj.isEmpty());
        QCOMPARE(obj.value(QStringLiteral("save_directory")).toString(),
                 QStringLiteral("/tmp/snaptest"));
        QVERIFY(obj.contains(QStringLiteral("recording")));
        QVERIFY(obj.contains(QStringLiteral("hotkeys")));
    }
};

QTEST_MAIN(TestPreferences)
#include "tst_preferences.moc"
