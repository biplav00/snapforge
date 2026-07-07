// State-machine + timing tests for RecordingManager, driven through the
// SnapforgeClient SEAM against the in-memory fake. No real FFI, no capture, no
// TCC grant — so the success / stop / pause paths that used to be untestable
// headlessly are now deterministic (sf::test::state() controls outcomes).
//
// GUILESS: RecordingManager uses a QTimer, which only needs a running
// QCoreApplication event loop, not widgets.

#include <QtTest/QtTest>
#include <QSignalSpy>
#include <QTemporaryDir>

#include "RecordingManager.h"
#include "SnapforgeClientTesting.h"

namespace {
// A config the fake returns from configLoadJson(), so reloadPrefs() has
// deterministic input. Mirrors the real on-disk schema (PascalCase enums).
QByteArray configWith(const QString &fmt, int fps, const QString &quality) {
    return QStringLiteral(
               R"({"recording":{"format":"%1","fps":%2,"quality":"%3"}})")
        .arg(fmt)
        .arg(fps)
        .arg(quality)
        .toUtf8();
}
} // namespace

class TestRecordingManager : public QObject {
    Q_OBJECT

private slots:
    void init() { sf::test::reset(); }

    void initialState_isIdle();
    void pauseResume_withoutHandle_areNoOps();
    void reloadPrefs_appliesConfigToOutputPath();
    void startRecording_success_emitsStartedAndSendsTypedRequest();
    void startRecording_failure_emitsError();
    void doubleStart_isRejectedWhenActive();
    void pauseResume_whenActive_emitSignals();
    void stop_success_emitsStopped();
    void stop_missingOutputFile_emitsError();
    void stop_whenIdle_emitsNothing();
};

void TestRecordingManager::initialState_isIdle() {
    RecordingManager mgr;
    QVERIFY(!mgr.isRecording());
    QVERIFY(!mgr.isPaused());
    QCOMPARE(mgr.elapsedSeconds(), 0);
    QVERIFY(mgr.outputPath().isEmpty());
}

void TestRecordingManager::pauseResume_withoutHandle_areNoOps() {
    RecordingManager mgr;
    QSignalSpy pausedSpy(&mgr, &RecordingManager::recordingPaused);
    QSignalSpy resumedSpy(&mgr, &RecordingManager::recordingResumed);

    mgr.pauseRecording();
    mgr.resumeRecording();

    QCOMPARE(pausedSpy.count(), 0);
    QCOMPARE(resumedSpy.count(), 0);
    QVERIFY(!mgr.isPaused());
    QVERIFY(!mgr.isRecording());
}

void TestRecordingManager::reloadPrefs_appliesConfigToOutputPath() {
    // reloadPrefs parses the recording prefs from the (faked) config; the
    // chosen format drives the output file extension on the next start.
    sf::test::state().configJson = configWith(QStringLiteral("Gif"), 60,
                                              QStringLiteral("High"));
    RecordingManager mgr;
    mgr.reloadPrefs();

    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());

    QVERIFY(mgr.isRecording());
    QVERIFY(mgr.outputPath().endsWith(QStringLiteral(".gif")));
    // The typed request the client received reflects the parsed prefs.
    QVERIFY(sf::test::state().lastRecordReq.has_value());
    QCOMPARE(sf::test::state().lastRecordReq->fps, 60u);
    QCOMPARE(sf::test::state().lastRecordReq->format, QStringLiteral("gif"));
    QCOMPARE(sf::test::state().lastRecordReq->quality, QStringLiteral("high"));
}

void TestRecordingManager::startRecording_success_emitsStartedAndSendsTypedRequest() {
    sf::test::state().recordStartSucceeds = true;
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(2, QRect(10, 20, 300, 400), dir.path());

    QCOMPARE(startedSpy.count(), 1);
    QCOMPARE(errorSpy.count(), 0);
    QVERIFY(mgr.isRecording());
    QVERIFY(mgr.outputPath().startsWith(dir.path()));

    // The seam received a typed request, not hand-built JSON.
    const auto &req = sf::test::state().lastRecordReq;
    QVERIFY(req.has_value());
    QCOMPARE(req->display, 2u);
    QVERIFY(req->region.has_value());
    QCOMPARE(req->region->width, 300u);
    QVERIFY(req->addToHistoryOnStop);
}

void TestRecordingManager::startRecording_failure_emitsError() {
    sf::test::state().recordStartSucceeds = false;
    sf::test::state().lastError = QStringLiteral("ffmpeg not found");
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());

    QCOMPARE(startedSpy.count(), 0);
    QCOMPARE(errorSpy.count(), 1);
    QVERIFY(!mgr.isRecording());
    // The FFI error message is surfaced to the user.
    QVERIFY(errorSpy.takeFirst().at(0).toString().contains(QStringLiteral("ffmpeg not found")));
}

void TestRecordingManager::doubleStart_isRejectedWhenActive() {
    sf::test::state().recordStartSucceeds = true;
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());

    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    // Rejected duplicate start is non-fatal: recordingWarning, NOT
    // recordingError — an error would make the controller leave recording
    // state (hide the stop control) while the real recording continues.
    QSignalSpy warningSpy(&mgr, &RecordingManager::recordingWarning);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    mgr.startRecording(0, QRect(), dir.path());
    QCOMPARE(warningSpy.count(), 1);
    QCOMPARE(errorSpy.count(), 0);
    QVERIFY(warningSpy.takeFirst().at(0).toString().contains(QStringLiteral("Already recording")));
    QVERIFY(mgr.isRecording());
}

void TestRecordingManager::pauseResume_whenActive_emitSignals() {
    sf::test::state().recordStartSucceeds = true;
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    QSignalSpy pausedSpy(&mgr, &RecordingManager::recordingPaused);
    QSignalSpy resumedSpy(&mgr, &RecordingManager::recordingResumed);

    mgr.pauseRecording();
    QCOMPARE(pausedSpy.count(), 1);
    QVERIFY(mgr.isPaused());

    mgr.resumeRecording();
    QCOMPARE(resumedSpy.count(), 1);
    QVERIFY(!mgr.isPaused());
}

void TestRecordingManager::stop_success_emitsStopped() {
    sf::test::state().recordStartSucceeds = true;
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    // stopRecording validates the real output file on disk (disk-full / moov
    // guards). The fake doesn't write one, so create a non-empty file at the
    // path the manager chose.
    QFile f(mgr.outputPath());
    QVERIFY(f.open(QIODevice::WriteOnly));
    f.write("not-empty");
    f.close();

    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    mgr.stopRecording();

    // Stop finalizes asynchronously (worker thread + queued signal back on
    // the main thread), so spin the event loop until the signal lands.
    QVERIFY(stoppedSpy.wait(5000));
    QCOMPARE(stoppedSpy.count(), 1);
    QCOMPARE(errorSpy.count(), 0);
    QVERIFY(!mgr.isRecording());
    QVERIFY(!mgr.isPaused());
}

void TestRecordingManager::stop_missingOutputFile_emitsError() {
    sf::test::state().recordStartSucceeds = true;
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    // No output file created -> the disk-full/missing guard fires even though
    // the FFI stop "succeeded".
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    mgr.stopRecording();

    // Async stop — wait for the queued error to land.
    QVERIFY(errorSpy.wait(5000));
    QCOMPARE(stoppedSpy.count(), 0);
    QCOMPARE(errorSpy.count(), 1);
    QVERIFY(!mgr.isRecording());
}

void TestRecordingManager::stop_whenIdle_emitsNothing() {
    RecordingManager mgr;
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    mgr.stopRecording();
    QCOMPARE(stoppedSpy.count(), 0);
    QCOMPARE(errorSpy.count(), 0);
}

QTEST_GUILESS_MAIN(TestRecordingManager)
#include "tst_recordingmanager.moc"
