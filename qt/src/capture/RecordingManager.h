#ifndef RECORDINGMANAGER_H
#define RECORDINGMANAGER_H

#include <QObject>
#include <QRect>
#include <QString>
#include <QTimer>
#include <QElapsedTimer>
#include <QFuture>
#include <atomic>

#include "SnapforgeClient.h"

class RecordingManager : public QObject {
    Q_OBJECT
public:
    explicit RecordingManager(QObject *parent = nullptr);
    ~RecordingManager();

    bool isRecording() const;
    bool isPaused() const;
    QString outputPath() const;
    int elapsedSeconds() const;

public slots:
    // Start recording. region is in pixel coords. If region is null rect, record fullscreen.
    void startRecording(int display, QRect region, QString outputDir);
    void stopRecording();
    void pauseRecording();
    void resumeRecording();

signals:
    void recordingStarted(QString outputPath);
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

private:
    void loadPrefsIfNeeded();

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

    // Cached recording prefs (M13). Reloaded on reloadPrefs().
    bool    m_prefsLoaded = false;
    QString m_prefFormat  = QStringLiteral("mp4");
    int     m_prefFps     = 30;
    QString m_prefQuality = QStringLiteral("medium");
};

#endif // RECORDINGMANAGER_H
