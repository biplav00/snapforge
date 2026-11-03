#ifndef RECORDINGMANAGER_H
#define RECORDINGMANAGER_H

#include <QObject>
#include <QRect>
#include <QString>
#include <QTimer>
#include <QElapsedTimer>
#include <QFuture>
#include <atomic>
#include <future>
#include <thread>
#include <vector>

#include "SnapforgeClient.h"

class RecordingManager : public QObject {
    Q_OBJECT
public:
    explicit RecordingManager(QObject *parent = nullptr);
    ~RecordingManager();

    bool isRecording() const;
    // Recording OR still finalizing a stopped recording. Callers that would
    // start a new capture (Record hotkey / tray) must check this, not
    // isRecording(): the handle is nulled the moment stop begins.
    bool isBusy() const;
    bool isStopping() const { return m_stopping; }
    bool isPaused() const;
    // The show-clicks pref actually applied to the current/last recording
    // (cached at start), as opposed to an unsaved Preferences checkbox.
    bool showClicksEnabled() const { return m_prefShowClicks; }
    QString outputPath() const;
    int elapsedSeconds() const;

public slots:
    // Start recording. region is in pixel coords. If region is null rect, record fullscreen.
    void startRecording(int display, QRect region, QString outputDir);
    void stopRecording();
    void pauseRecording();
    void resumeRecording();
    // System sleep/wake hooks (WorkspaceSleepObserver). Sleep pauses only an
    // actively-running recording and remembers that *we* paused it; wake
    // resumes only in that case, so a manual pause survives a sleep/wake cycle.
    void handleSystemSleep();
    void handleSystemWake();

signals:
    void recordingStarted(QString outputPath);
    // stopRecording() handed the session to the finalize worker; the UI should
    // show a "Finalizing…" state until recordingStopped/recordingError.
    void recordingFinalizing();
    void recordingStopped(QString outputPath);
    void recordingPaused();
    void recordingResumed();
    void elapsedChanged(int seconds);
    // Fatal: recording never started or is no longer running (m_handle gone).
    // UI should reset to idle.
    void recordingError(QString message);
    // Non-fatal: a pause/resume FFI call failed but m_handle is still alive
    // and frames keep writing to disk. UI must KEEP its recording state —
    // resetting to idle here would hide the stop control mid-recording.
    void recordingWarning(QString message);

public slots:
    // Reload cached recording prefs from snapforge_config. Connect this to
    // PreferencesWindow::configSaved so we don't parse JSON on every start.
    void reloadPrefs();

public:
    // UI fallback deadline for an interactive stop (default 30s). If the
    // finalize worker hasn't reported back by then, recordingError is emitted
    // and the start gate re-opens; the stuck worker is kept and reaped later
    // (or bounded by the dtor deadline). Exposed for tests.
    void setStopTimeoutMs(int ms) { m_stopTimeoutMs = ms; }

private:
    void loadPrefsIfNeeded();
    void onTick();
    // Main-thread continuation of stopRecording(): validates the output and
    // emits recordingStopped/recordingError once the worker's recordStop is
    // done. `gen` identifies the stop; a result for a stop the watchdog has
    // already given up on is logged and dropped.
    void finishStop(quint64 gen, bool ok, const QString &err, const QString &path);
    void onStopTimeout();
    // Join every stop worker that has already finished (non-blocking).
    void reapStopJobs();

    // Atomic so dtor and stopRecording cannot both observe the same non-null
    // pointer and end up double-stopping / double-freeing it. Whoever swaps
    // it to nullptr first owns the teardown. We store the raw sf::RecHandle
    // pointer (RecHandle is a thin void* wrapper) so the swap stays lock-free;
    // call sites wrap it back into sf::RecHandle{} before calling sf::record*.
    std::atomic<void *> m_handle{nullptr};
    QString m_outputPath;
    QTimer *m_timer = nullptr;
    QElapsedTimer m_elapsed;
    qint64 m_accumulatedMs = 0; // total elapsed ms across pauses
    bool m_paused = false;
    // Set only by handleSystemSleep when it auto-pauses; a manual
    // pause/resume/stop (or a new start) clears it.
    bool m_autoPausedBySleep = false;

    // Interactive stop runs recordStop on a worker so a slow ffmpeg finalize
    // can't beachball the GUI thread. m_stopping gates startRecording while
    // the finalize is in flight (the handle is already nulled, so isRecording
    // alone would let a new start through). Normally there is at most one
    // job; a worker the watchdog gave up on stays here until it finishes. The
    // dtor waits on all of them under the same 30s deadline it applies to its
    // own in-place stop.
    struct StopJob {
        quint64 gen = 0;
        std::thread worker;
        std::shared_future<void> done;
    };
    bool m_stopping = false;
    quint64 m_stopGen = 0;        // generation of the stop currently awaited
    std::vector<StopJob> m_stopJobs;
    QTimer *m_stopWatchdog = nullptr;
    int m_stopTimeoutMs = 40000; // > Rust worst case (25s stop deadline + 5s kill grace + indexing)

    // Cached recording prefs (M13). Reloaded on reloadPrefs().
    bool    m_prefsLoaded = false;
    QString m_prefFormat  = QStringLiteral("mp4");
    int     m_prefFps     = 30;
    QString m_prefQuality = QStringLiteral("medium");
    bool    m_prefShowClicks = false;
};

#endif // RECORDINGMANAGER_H
