// ClickTap through the SnapforgeClient seam (fake adapter). Proves the seam's
// payoff: the click path is now unit-testable with no real event tap and no
// Input Monitoring TCC grant — sf::test::fireClick simulates the Rust tap.

#include <QtTest/QtTest>
#include <QSignalSpy>

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

    void click_emitsClickedSignalWithMappedArgs() {
        ClickTap tap;
        QVERIFY(tap.start());
        QSignalSpy spy(&tap, &ClickTap::clicked);

        // Simulate a right mouse-down arriving from the (faked) tap thread.
        sf::test::fireClick(120.0, 240.0, /*rightClick=*/true);

        // onClickStatic re-dispatches via Qt::QueuedConnection; wait() pumps
        // the event loop until the queued emit runs.
        QVERIFY(spy.wait(1000));
        QCOMPARE(spy.count(), 1);
        const QList<QVariant> args = spy.takeFirst();
        QCOMPARE(args.at(0).toPoint(), QPoint(120, 240));
        QCOMPARE(args.at(1).toBool(), true);
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
