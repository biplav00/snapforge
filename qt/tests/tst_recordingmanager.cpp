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
#include <atomic>
#include <chrono>
#include <thread>

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
    void start_rejectedWhileStopping();
    void stop_emitsFinalizingAndIsBusyUntilDone();
    void deadWorker_detectedByTimer_emitsErrorAndGoesIdle();
    void pauseFailure_emitsWarningNotError();
    void resumeFailure_emitsWarningNotError();
    void stopWatchdog_firesAndReopensStartGate();
    void sleepWake_autoResumesOnlyAutoPause();
    void outputPath_isUniqueForBackToBackStarts();
    void showClicksEnabled_reflectsSavedConfig();
    void outputPath_followsCoreUniquifiedPath();
};

namespace {
// Holds the fake recordStop on the worker thread until released.
struct StopGate {
    std::atomic<bool> released{false};
    void install() {
        sf::test::state().recordStopHook = [this]() {
            while (!released.load())
                std::this_thread::sleep_for(std::chrono::milliseconds(2));
        };
    }
    void release() { released.store(true); }
};
} // namespace

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

void TestRecordingManager::start_rejectedWhileStopping() {
    StopGate gate;
    gate.install();
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    mgr.stopRecording();
    QVERIFY(!mgr.isRecording());
    QVERIFY(mgr.isBusy());

    QSignalSpy warningSpy(&mgr, &RecordingManager::recordingWarning);
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    mgr.startRecording(0, QRect(), dir.path());
    QCOMPARE(startedSpy.count(), 0);
    QCOMPARE(warningSpy.count(), 1);
    QVERIFY(warningSpy.takeFirst().at(0).toString().contains(QStringLiteral("finalizing")));

    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    gate.release();
    QVERIFY(errorSpy.wait(5000)); // no output file -> "empty or missing"
    QVERIFY(!mgr.isBusy());
}

void TestRecordingManager::stop_emitsFinalizingAndIsBusyUntilDone() {
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QFile f(mgr.outputPath());
    QVERIFY(f.open(QIODevice::WriteOnly));
    f.write("x");
    f.close();

    QSignalSpy finalizingSpy(&mgr, &RecordingManager::recordingFinalizing);
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    mgr.stopRecording();
    QCOMPARE(finalizingSpy.count(), 1);
    QVERIFY(mgr.isBusy());
    QVERIFY(stoppedSpy.wait(5000));
    QVERIFY(!mgr.isBusy());
}

void TestRecordingManager::deadWorker_detectedByTimer_emitsErrorAndGoesIdle() {
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.isRecording());

    // The Rust worker dies on its own; stop then reports its real error.
    sf::test::state().recordAlive = false;
    sf::test::state().recordStopSucceeds = false;
    sf::test::state().recordStopError = QStringLiteral("ffmpeg exited with status 1");

    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    QSignalSpy finalizingSpy(&mgr, &RecordingManager::recordingFinalizing);
    // The 1s tick polls recordIsAlive.
    QVERIFY(errorSpy.wait(5000));
    QCOMPARE(finalizingSpy.count(), 1);
    QVERIFY(errorSpy.takeFirst().at(0).toString().contains(QStringLiteral("ffmpeg exited")));
    QVERIFY(!mgr.isRecording());
    QVERIFY(!mgr.isBusy());
    QCOMPARE(sf::test::state().recordStopCalls, 1);
}

void TestRecordingManager::pauseFailure_emitsWarningNotError() {
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    sf::test::state().recordPauseSucceeds = false;

    QSignalSpy warningSpy(&mgr, &RecordingManager::recordingWarning);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    QSignalSpy pausedSpy(&mgr, &RecordingManager::recordingPaused);
    mgr.pauseRecording();
    QCOMPARE(warningSpy.count(), 1);
    QCOMPARE(errorSpy.count(), 0);
    QCOMPARE(pausedSpy.count(), 0);
    QVERIFY(!mgr.isPaused());
    QVERIFY(mgr.isRecording());
}

void TestRecordingManager::resumeFailure_emitsWarningNotError() {
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    mgr.pauseRecording();
    QVERIFY(mgr.isPaused());
    sf::test::state().recordResumeSucceeds = false;

    QSignalSpy warningSpy(&mgr, &RecordingManager::recordingWarning);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
    mgr.resumeRecording();
    QCOMPARE(warningSpy.count(), 1);
    QCOMPARE(errorSpy.count(), 0);
    QVERIFY(mgr.isPaused());
    QVERIFY(mgr.isRecording());
}

void TestRecordingManager::stopWatchdog_firesAndReopensStartGate() {
    StopGate gate;
    gate.install();
    {
        RecordingManager mgr;
        mgr.setStopTimeoutMs(100);
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        mgr.startRecording(0, QRect(), dir.path());

        QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);
        QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
        mgr.stopRecording();
        QVERIFY(mgr.isBusy());
        QVERIFY(errorSpy.wait(5000));
        QVERIFY(errorSpy.takeFirst().at(0).toString().contains(QStringLiteral("too long")));
        QVERIFY(!mgr.isBusy());

        // A new recording can start while the wedged worker is still stuck.
        sf::test::state().recordStopHook = nullptr; // only affects new stops
        QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
        mgr.startRecording(0, QRect(), dir.path());
        QCOMPARE(startedSpy.count(), 1);
        QVERIFY(mgr.isRecording());

        // The wedged worker finishing late must not touch the new session.
        gate.release();
        QTest::qWait(200);
        QCOMPARE(stoppedSpy.count(), 0);
        QCOMPARE(errorSpy.count(), 0);
        QVERIFY(mgr.isRecording());
    } // dtor joins the (now finished) late worker and stops the new session
}

void TestRecordingManager::sleepWake_autoResumesOnlyAutoPause() {
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());

    // Running -> sleep pauses, wake resumes.
    mgr.handleSystemSleep();
    QVERIFY(mgr.isPaused());
    mgr.handleSystemWake();
    QVERIFY(!mgr.isPaused());

    // Manual pause survives a sleep/wake cycle.
    mgr.pauseRecording();
    QVERIFY(mgr.isPaused());
    mgr.handleSystemSleep();
    mgr.handleSystemWake();
    QVERIFY(mgr.isPaused());

    // A failed auto-pause doesn't arm auto-resume.
    mgr.resumeRecording();
    sf::test::state().recordPauseSucceeds = false;
    mgr.handleSystemSleep();
    QVERIFY(!mgr.isPaused());
    sf::test::state().recordPauseSucceeds = true;
    mgr.pauseRecording();
    mgr.handleSystemWake();
    QVERIFY(mgr.isPaused());
}

void TestRecordingManager::outputPath_isUniqueForBackToBackStarts() {
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    RecordingManager mgr;
    mgr.startRecording(0, QRect(), dir.path());
    const QString first = mgr.outputPath();
    QFile f(first);
    QVERIFY(f.open(QIODevice::WriteOnly));
    f.write("x");
    f.close();
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    mgr.stopRecording();
    QVERIFY(stoppedSpy.wait(5000));

    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.outputPath() != first);
    QVERIFY(!QFile::exists(mgr.outputPath()));
}

void TestRecordingManager::showClicksEnabled_reflectsSavedConfig() {
    sf::test::state().configJson =
        QByteArrayLiteral(R"({"recording":{"show_clicks":true}})");
    RecordingManager mgr;
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    mgr.startRecording(0, QRect(), dir.path());
    QVERIFY(mgr.showClicksEnabled());
    QVERIFY(sf::test::state().lastRecordReq->showClicks);
}

void TestRecordingManager::outputPath_followsCoreUniquifiedPath() {
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString real = dir.path() + QStringLiteral("/recording_core-1.mp4");
    sf::test::state().recordOutputPathOverride = real;
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    mgr.startRecording(0, QRect(), dir.path());
    QCOMPARE(mgr.outputPath(), real);
    QCOMPARE(startedSpy.takeFirst().at(0).toString(), real);

    QFile f(real);
    QVERIFY(f.open(QIODevice::WriteOnly));
    f.write("x");
    f.close();
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    mgr.stopRecording();
    QVERIFY(stoppedSpy.wait(5000));
    QCOMPARE(stoppedSpy.takeFirst().at(0).toString(), real);
}

QTEST_GUILESS_MAIN(TestRecordingManager)
#include "tst_recordingmanager.moc"
