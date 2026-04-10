#include "RecordingManager.h"

#include <QDateTime>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>

#include "snapforge_ffi.h"

RecordingManager::RecordingManager(QObject *parent)
    : QObject(parent)
    , m_timer(new QTimer(this))
{
    m_timer->setInterval(1000);
    connect(m_timer, &QTimer::timeout, this, [this]() {
        emit elapsedChanged(elapsedSeconds());
    });
}

RecordingManager::~RecordingManager()
{
    if (isRecording()) {
        snapforge_stop_recording(m_handle);
    }
    if (m_handle) {
        snapforge_free_recording_handle(m_handle);
        m_handle = nullptr;
    }
}

bool RecordingManager::isRecording() const
{
    return m_handle != nullptr && snapforge_is_recording(m_handle) == 1;
}

QString RecordingManager::outputPath() const
{
    return m_outputPath;
}

int RecordingManager::elapsedSeconds() const
{
    return static_cast<int>(m_elapsed.elapsed() / 1000);
}

void RecordingManager::startRecording(int display, QRect region, QString outputDir)
{
    if (isRecording()) {
        emit recordingError(QStringLiteral("Already recording"));
        return;
    }

    // Generate timestamped output path
    QString timestamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd_HH-mm-ss"));
    m_outputPath = outputDir + QStringLiteral("/recording_") + timestamp + QStringLiteral(".mp4");

    // Build JSON config
    QJsonObject config;
    config[QStringLiteral("display")] = display;
    config[QStringLiteral("output_path")] = m_outputPath;
    config[QStringLiteral("format")] = QStringLiteral("mp4");
    config[QStringLiteral("fps")] = 30;
    config[QStringLiteral("quality")] = QStringLiteral("medium");
    config[QStringLiteral("ffmpeg_path")] = QJsonValue::Null;

    if (!region.isNull()) {
        QJsonObject regionObj;
        regionObj[QStringLiteral("x")] = region.x();
        regionObj[QStringLiteral("y")] = region.y();
        regionObj[QStringLiteral("width")] = region.width();
        regionObj[QStringLiteral("height")] = region.height();
        config[QStringLiteral("region")] = regionObj;
    } else {
        config[QStringLiteral("region")] = QJsonValue::Null;
    }

    QByteArray jsonBytes = QJsonDocument(config).toJson(QJsonDocument::Compact);

    m_handle = snapforge_start_recording(jsonBytes.constData());
    if (!m_handle) {
        emit recordingError(QStringLiteral("Failed to start recording"));
        return;
    }

    m_elapsed.start();
    m_timer->start();

    emit recordingStarted(m_outputPath);
}

void RecordingManager::stopRecording()
{
    if (!m_handle) {
        return;
    }

    m_timer->stop();

    snapforge_stop_recording(m_handle);
    snapforge_free_recording_handle(m_handle);
    m_handle = nullptr;

    snapforge_history_add(m_outputPath.toUtf8().constData());

    emit recordingStopped(m_outputPath);
}
