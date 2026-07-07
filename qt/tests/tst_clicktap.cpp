// ClickTap through the SnapforgeClient seam (fake adapter). Proves the seam's
// payoff: the permission probe is now unit-testable with no real event tap and
// no Input Monitoring TCC grant.

#include <QtTest/QtTest>

#include "ClickTap.h"
#include "SnapforgeClientTesting.h"

class TestClickTap : public QObject {
    Q_OBJECT
private slots:
    void init() { sf::test::reset(); }

    void start_failsWhenTapRefused() {
        sf::test::state().clicksStartSucceeds = false;
        ClickTap tap;
        QVERIFY(!tap.start());
        QVERIFY(!tap.isRunning());
    }

    void start_succeedsAndStreams() {
        ClickTap tap;
        QVERIFY(tap.start());
        QVERIFY(tap.isRunning());
        QVERIFY(sf::test::state().clicksRunning);
    }

    void stop_endsStreaming() {
        ClickTap tap;
        QVERIFY(tap.start());
        tap.stop();
        QVERIFY(!tap.isRunning());
        QVERIFY(!sf::test::state().clicksRunning);
    }
};

QTEST_MAIN(TestClickTap)
#include "tst_clicktap.moc"
