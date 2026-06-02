// State-machine + timing tests for RecordingManager.
//
// RecordingManager owns the recording lifecycle on the Qt side; the actual
// capture happens in Rust behind the FFI. We test the parts that DON'T need a
// real capture session:
//   * initial idle state (no handle, zero elapsed, not paused),
//   * pause/resume guards when there is no active handle (must be no-ops, not
//     crashes — they early-return on a null handle),
//   * startRecording rejecting a second start while "recording",
//   * startRecording failing cleanly (recordingError) when capture is
//     unavailable headlessly — no display / TCC grant required.
//
// GUILESS: RecordingManager uses a QTimer, which only needs a running
// QCoreApplication event loop, not widgets.

#include <QtTest/QtTest>
#include <QSignalSpy>
#include <QTemporaryDir>

#include "RecordingManager.h"

class TestRecordingManager : public QObject {
    Q_OBJECT

private slots:
    void initialState_isIdle();
    void pauseResume_withoutHandle_areNoOps();
    void reloadPrefs_doesNotCrash_andStaysIdle();
    void startRecording_withoutCapture_emitsErrorOrStarts();
    void doubleStart_isRejectedWhenActive();
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

    // No active handle -> both must early-return, emit nothing, and leave the
    // manager idle (no null-deref into the FFI).
    mgr.pauseRecording();
    mgr.resumeRecording();

    QCOMPARE(pausedSpy.count(), 0);
    QCOMPARE(resumedSpy.count(), 0);
    QVERIFY(!mgr.isPaused());
    QVERIFY(!mgr.isRecording());
}

void TestRecordingManager::reloadPrefs_doesNotCrash_andStaysIdle() {
    // reloadPrefs reads the shared config via FFI and parses recording prefs.
    // It must succeed whether or not a config file exists (defaults apply) and
    // must not change the idle lifecycle state.
    RecordingManager mgr;
    mgr.reloadPrefs();
    QVERIFY(!mgr.isRecording());
    QCOMPARE(mgr.elapsedSeconds(), 0);
}

void TestRecordingManager::startRecording_withoutCapture_emitsErrorOrStarts() {
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    QTemporaryDir dir;
    QVERIFY(dir.isValid());

    // Fullscreen (null region). On a headless runner with no capture
    // permission this fails and surfaces recordingError; on a real desktop it
    // may actually start. Either way exactly one of the two outcomes fires and
    // the manager's reported state agrees with it — no silent limbo.
    mgr.startRecording(0, QRect(), dir.path());

    const int started = startedSpy.count();
    const int errored = errorSpy.count();
    QCOMPARE(started + errored, 1);

    if (started == 1) {
        QVERIFY(mgr.isRecording());
        // The generated path lands under the requested dir with the format ext.
        QVERIFY(mgr.outputPath().startsWith(dir.path()));
        mgr.stopRecording();
    } else {
        QVERIFY(!mgr.isRecording());
    }
}

void TestRecordingManager::doubleStart_isRejectedWhenActive() {
    RecordingManager mgr;
    QSignalSpy startedSpy(&mgr, &RecordingManager::recordingStarted);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    QTemporaryDir dir;
    QVERIFY(dir.isValid());

    mgr.startRecording(0, QRect(), dir.path());
    if (!mgr.isRecording()) {
        // Capture unavailable headlessly; the "already recording" guard can't
        // be exercised here. Not a failure of the unit under test.
        QSKIP("capture did not start (headless); cannot exercise double-start guard");
    }

    errorSpy.clear();
    // Second start while active must be rejected with an "Already recording"
    // error and must not start a new session.
    mgr.startRecording(0, QRect(), dir.path());
    QCOMPARE(errorSpy.count(), 1);
    QVERIFY(mgr.isRecording());

    mgr.stopRecording();
}

void TestRecordingManager::stop_whenIdle_emitsNothing() {
    RecordingManager mgr;
    QSignalSpy stoppedSpy(&mgr, &RecordingManager::recordingStopped);
    QSignalSpy errorSpy(&mgr, &RecordingManager::recordingError);

    // stopRecording on an idle manager atomically observes a null handle and
    // returns immediately — no stopped/error signal.
    mgr.stopRecording();
    QCOMPARE(stoppedSpy.count(), 0);
    QCOMPARE(errorSpy.count(), 0);
}

QTEST_GUILESS_MAIN(TestRecordingManager)
#include "tst_recordingmanager.moc"
