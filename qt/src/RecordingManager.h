#ifndef RECORDINGMANAGER_H
#define RECORDINGMANAGER_H

#include <QObject>
#include <QRect>
#include <QString>
#include <QTimer>
#include <QElapsedTimer>

class RecordingManager : public QObject {
    Q_OBJECT
public:
    explicit RecordingManager(QObject *parent = nullptr);
    ~RecordingManager();

    bool isRecording() const;
    QString outputPath() const;
    int elapsedSeconds() const;

public slots:
    // Start recording. region is in pixel coords. If region is null rect, record fullscreen.
    void startRecording(int display, QRect region, QString outputDir);
    void stopRecording();

signals:
    void recordingStarted(QString outputPath);
    void recordingStopped(QString outputPath);
    void elapsedChanged(int seconds);
    void recordingError(QString message);

private:
    void *m_handle = nullptr;
    QString m_outputPath;
    QTimer *m_timer = nullptr;
    QElapsedTimer m_elapsed;
};

#endif // RECORDINGMANAGER_H
