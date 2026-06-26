// Smoke test of the REAL SnapforgeClient adapter (SnapforgeClient.cpp) against
// the real Rust staticlib, on paths that work headless with no capture / TCC
// grant. Confirms the adapter compiles, links, and round-trips the FFI; the
// fake-driven behavioural tests live in the per-unit tst_* files.

#include <QtTest/QtTest>

#include "SnapforgeClient.h"

class TestSnapforgeClient : public QObject {
    Q_OBJECT
private slots:
    void lastErrorCode_startsNone() {
        // No use-case call has failed in this fresh process.
        QCOMPARE(sf::lastErrorCode(), 0);
    }

    void configLoad_returnsJsonObject() {
        const QByteArray json = sf::configLoadJson();
        QVERIFY(!json.isEmpty());
        QVERIFY(json.trimmed().startsWith('{'));
    }

    void defaultSavePath_nonEmpty() {
        QVERIFY(!sf::defaultSavePath().isEmpty());
    }

    void isIncompleteMp4_nonMp4IsFalse() {
        QVERIFY(!sf::isIncompleteMp4(QStringLiteral("/tmp/not-a-video.txt")));
    }
};

QTEST_MAIN(TestSnapforgeClient)
#include "tst_snapforgeclient.moc"
