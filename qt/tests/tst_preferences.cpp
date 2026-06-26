// PreferencesWindow config round-trip through the SnapforgeClient SEAM (fake).
// Proves the migrated config load/save path: a known config returned by the
// fake lands in the widgets, and a save serializes back through the seam. No
// real FFI, no config file on disk. Runs offscreen (QT_QPA_PLATFORM set by the
// CMake test ENVIRONMENT) so it needs no window server.

#include <QtTest/QtTest>
#include <QSignalSpy>
#include <QJsonDocument>
#include <QJsonObject>

#include "PreferencesWindow.h"
#include "SnapforgeClientTesting.h"

namespace {
QByteArray knownConfig() {
    return QByteArrayLiteral(R"({
        "save_directory": "/tmp/snaptest",
        "auto_copy_clipboard": true,
        "show_notification": true,
        "remember_last_region": false,
        "screenshot_format": "Jpg",
        "jpg_quality": 75,
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
