#ifndef RECORDINGMANAGER_H
#define RECORDINGMANAGER_H

#include <QObject>
#include <QRect>
#include <QString>
#include <QTimer>
#include <QElapsedTimer>
#include <QFuture>

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
    void recordingError(QString message);

public slots:
    // Reload cached recording prefs from snapforge_config. Connect this to
    // PreferencesWindow::configSaved so we don't parse JSON on every start.
    void reloadPrefs();

private:
    void loadPrefsIfNeeded();

    void *m_handle = nullptr;
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
