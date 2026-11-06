#include <QtTest>

#include "RecordingBorderWindow.h"

// The border's native capture exclusion is macOS-only and visual, but its
// geometry/state contract is testable offscreen: the window must be expanded
// so the stroke sits outside the recorded pixels, and an empty (fullscreen)
// region must never show a border.
class TestRecordingBorder : public QObject {
    Q_OBJECT
private slots:
    void emptyRegionClears();
    void expandsRegionForStroke();
    void showsOnlyWhenPrepared();
    void clearHidesAndForgetsRegion();
};

void TestRecordingBorder::emptyRegionClears() {
    RecordingBorderWindow w;
    w.setRegion(QRect(10, 20, 300, 200));
    w.setRegion(QRect());
    w.showForPreparedRegion();
    QVERIFY(!w.isVisible());
}

void TestRecordingBorder::expandsRegionForStroke() {
    RecordingBorderWindow w;
    w.setRegion(QRect(100, 200, 300, 400));
    // 3px outward on each side, so the 2px stroke lands outside the region.
    QCOMPARE(w.geometry(), QRect(97, 197, 306, 406));
}

void TestRecordingBorder::showsOnlyWhenPrepared() {
    RecordingBorderWindow w;
    w.showForPreparedRegion();
    QVERIFY(!w.isVisible());

    w.setRegion(QRect(50, 60, 640, 480));
    w.showForPreparedRegion();
    QVERIFY(w.isVisible());
}

void TestRecordingBorder::clearHidesAndForgetsRegion() {
    RecordingBorderWindow w;
    w.setRegion(QRect(0, 0, 100, 100));
    w.showForPreparedRegion();
    QVERIFY(w.isVisible());

    w.clear();
    QVERIFY(!w.isVisible());

    // A cleared region is not re-shown by a stray started signal.
    w.showForPreparedRegion();
    QVERIFY(!w.isVisible());
}

QTEST_MAIN(TestRecordingBorder)
#include "tst_recordingborder.moc"
