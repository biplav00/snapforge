#include "RecordingManager.h"

#include <QDateTime>
#include <QDebug>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <future>
#include <chrono>

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
    // H5: bound shutdown to 5 seconds.
    // snapforge_stop_recording blocks until ffmpeg finalizes the file which
    // can be slow. Run it on a detached std::thread and wait via a promise.
    if (!m_handle) {
        return;
    }
    void *handle = m_handle;
    m_handle = nullptr;

    auto donePromise = std::make_shared<std::promise<void>>();
    std::future<void> doneFuture = donePromise->get_future();

    std::thread([handle, donePromise]() {
        snapforge_stop_recording(handle);
        snapforge_free_recording_handle(handle);
        donePromise->set_value();
    }).detach();

    if (doneFuture.wait_for(std::chrono::seconds(5)) != std::future_status::ready) {
        qWarning("RecordingManager: shutdown timed out after 5s; detaching stop thread");
        // Thread keeps running and finishes whenever ffmpeg exits — the process
        // is going away anyway, so a brief leak during teardown is acceptable.
    }
}

bool RecordingManager::isRecording() const
{
    return m_handle != nullptr && snapforge_is_recording(m_handle) == 1;
}

bool RecordingManager::isPaused() const
{
    return m_paused;
}

QString RecordingManager::outputPath() const
{
    return m_outputPath;
}

int RecordingManager::elapsedSeconds() const
{
    // While running, accumulated + current lap. While paused, only accumulated.
    qint64 total = m_accumulatedMs;
    if (!m_paused && m_elapsed.isValid()) {
        total += m_elapsed.elapsed();
    }
    return static_cast<int>(total / 1000);
}

void RecordingManager::pauseRecording()
{
    if (!m_handle || m_paused) return;
    if (snapforge_pause_recording(m_handle) != 0) {
        emit recordingError(QStringLiteral("Failed to pause recording"));
        return;
    }
    // Bank the current lap into accumulated, stop ticking the current lap.
    if (m_elapsed.isValid()) {
        m_accumulatedMs += m_elapsed.elapsed();
    }
    m_elapsed.invalidate();
    m_paused = true;
    emit recordingPaused();
}

void RecordingManager::resumeRecording()
{
    if (!m_handle || !m_paused) return;
    if (snapforge_resume_recording(m_handle) != 0) {
        emit recordingError(QStringLiteral("Failed to resume recording"));
        return;
    }
    m_elapsed.start();
    m_paused = false;
    emit recordingResumed();
}

void RecordingManager::loadPrefsIfNeeded()
{
    if (m_prefsLoaded) return;
    reloadPrefs();
}

void RecordingManager::reloadPrefs()
{
    // Defaults
    m_prefFormat  = QStringLiteral("mp4");
    m_prefFps     = 30;
    m_prefQuality = QStringLiteral("medium");

    if (char *cfgJson = snapforge_config_load()) {
        QJsonDocument doc = QJsonDocument::fromJson(QByteArray(cfgJson));
        snapforge_free_string(cfgJson);
        QJsonObject rec = doc.object().value(QStringLiteral("recording")).toObject();
        // Rust serializes enums as unit variants ("Mp4"/"Gif", "Low"/"Medium"/"High").
        QString recFmt = rec.value(QStringLiteral("format")).toString();
        if (recFmt.compare(QStringLiteral("Gif"), Qt::CaseInsensitive) == 0) {
            m_prefFormat = QStringLiteral("gif");
        } else if (recFmt.compare(QStringLiteral("Mp4"), Qt::CaseInsensitive) == 0) {
            m_prefFormat = QStringLiteral("mp4");
        }
        int recFps = rec.value(QStringLiteral("fps")).toInt(0);
        if (recFps >= 1 && recFps <= 240) {
            m_prefFps = recFps;
        }
        QString recQual = rec.value(QStringLiteral("quality")).toString();
        if (recQual.compare(QStringLiteral("Low"), Qt::CaseInsensitive) == 0) {
            m_prefQuality = QStringLiteral("low");
        } else if (recQual.compare(QStringLiteral("High"), Qt::CaseInsensitive) == 0) {
            m_prefQuality = QStringLiteral("high");
        } else if (recQual.compare(QStringLiteral("Medium"), Qt::CaseInsensitive) == 0) {
            m_prefQuality = QStringLiteral("medium");
        }
    }
    m_prefsLoaded = true;
}

void RecordingManager::startRecording(int display, QRect region, QString outputDir)
{
    if (isRecording()) {
        emit recordingError(QStringLiteral("Already recording"));
        return;
    }

    loadPrefsIfNeeded();
    QString fmt     = m_prefFormat;
    int     fps     = m_prefFps;
    QString quality = m_prefQuality;

    // Generate timestamped output path with extension matching selected format.
    QString timestamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd_HH-mm-ss"));
    m_outputPath = outputDir + QStringLiteral("/recording_") + timestamp + QStringLiteral(".") + fmt;

    // Build JSON config
    QJsonObject config;
    config[QStringLiteral("display")] = display;
    config[QStringLiteral("output_path")] = m_outputPath;
    config[QStringLiteral("format")] = fmt;
    config[QStringLiteral("fps")] = fps;
    config[QStringLiteral("quality")] = quality;
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
        QString detail = QStringLiteral("Failed to start recording");
        if (char *errMsg = snapforge_last_recording_error()) {
            detail = QStringLiteral("Recording failed: %1").arg(QString::fromUtf8(errMsg));
            snapforge_free_string(errMsg);
        }
        emit recordingError(detail);
        return;
    }

    m_accumulatedMs = 0;
    m_paused = false;
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

    int rc = snapforge_stop_recording(m_handle);
    snapforge_free_recording_handle(m_handle);
    m_handle = nullptr;

    if (rc != 0) {
        QString detail = QStringLiteral("Recording failed during finalization");
        if (char *errMsg = snapforge_last_recording_error()) {
            detail = QStringLiteral("Recording failed: %1").arg(QString::fromUtf8(errMsg));
            snapforge_free_string(errMsg);
        }
        emit recordingError(detail);
        return;
    }

    // Guard against "success but the file is empty / gone" — happens when
    // the volume runs out of space mid-record, or when the mp4 is missing its
    // moov atom (truncated container). Delegate the structural check to Rust.
    QFileInfo fi(m_outputPath);
    if (!fi.exists() || fi.size() == 0) {
        emit recordingError(QStringLiteral("Recording output is empty or missing (disk full?)"));
        return;
    }
    if (snapforge_is_incomplete_mp4(m_outputPath.toUtf8().constData()) == 1) {
        emit recordingError(QStringLiteral("Recording output is corrupt (missing moov atom)"));
        return;
    }

    int addRc = snapforge_history_add(m_outputPath.toUtf8().constData());
    if (addRc == -2) {
        // Benign: Rust detected an incomplete mp4 and skipped indexing. This
        // shouldn't normally happen here since we already checked above, but
        // handle it defensively as a warning (no error signal).
        qWarning("RecordingManager: history skipped incomplete mp4: %s",
                 qUtf8Printable(m_outputPath));
    } else if (addRc == -1) {
        emit recordingError(QStringLiteral("Failed to add recording to history"));
        return;
    }

    emit recordingStopped(m_outputPath);
}
