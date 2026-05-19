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
#include <cstdlib>

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
    // snapforge_record_stop blocks until ffmpeg finalizes the file which can
    // be slow (writing the moov atom for a long recording). Run it on a worker
    // thread so we can apply a deadline. Atomically grab ownership of the
    // handle: if stopRecording() races us, the exchange leaves us with nullptr
    // and we just return.
    void *handle = m_handle.exchange(nullptr);
    if (!handle) {
        return;
    }

    auto donePromise = std::make_shared<std::promise<void>>();
    std::future<void> doneFuture = donePromise->get_future();

    std::thread worker([handle, donePromise]() {
        snapforge_record_stop(handle);
        snapforge_record_free_handle(handle);
        donePromise->set_value();
    });

    // 30s gives ffmpeg enough headroom for moov finalisation on long
    // recordings without making app quit feel hung in the normal case where
    // it finishes well under 1s.
    if (doneFuture.wait_for(std::chrono::seconds(30)) == std::future_status::ready) {
        worker.join();
        return;
    }

    // Previously we detached and let main() return, but the detached thread
    // kept calling into Rust statics (LAST_RECORDING_ERROR etc.) while the
    // process was tearing them down — undefined behaviour. _Exit terminates
    // the process immediately without running further destructors or atexit
    // hooks, which is strictly safer than racing teardown order.
    qWarning("RecordingManager: ffmpeg finalize did not complete within 30s; "
             "aborting via _Exit to avoid FFI use-after-teardown");
    worker.detach();
    std::_Exit(1);
}

bool RecordingManager::isRecording() const
{
    // The use-case FFI doesn't expose an is_recording probe; the Qt wrapper
    // owns the lifecycle (m_handle is atomically nulled on stop / error), so
    // pointer-non-null is the authoritative liveness signal. If the Rust
    // worker dies asynchronously, RecordingManager::recordingError surfaces
    // it via the controller and the UI returns to idle.
    return m_handle.load() != nullptr;
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
    void *h = m_handle.load();
    if (!h || m_paused) return;
    if (snapforge_record_pause(h) != 0) {
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
    void *h = m_handle.load();
    if (!h || !m_paused) return;
    if (snapforge_record_resume(h) != 0) {
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

    // Use-case FFI handles history indexing on successful stop. Qt still does
    // its own existence/incomplete-mp4 sanity checks below so the user gets a
    // specific error message instead of a silent skip.
    config[QStringLiteral("add_to_history_on_stop")] = true;

    QByteArray jsonBytes = QJsonDocument(config).toJson(QJsonDocument::Compact);

    void *newHandle = snapforge_record_start(jsonBytes.constData());
    m_handle.store(newHandle);
    if (!newHandle) {
        QString detail = QStringLiteral("Failed to start recording");
        if (char *errMsg = snapforge_app_last_error()) {
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

    qInfo("Recording started: display=%d region=%dx%d format=%s fps=%d quality=%s -> %s",
          display, region.width(), region.height(),
          qUtf8Printable(fmt), fps, qUtf8Printable(quality),
          qUtf8Printable(m_outputPath));
    emit recordingStarted(m_outputPath);
}

void RecordingManager::stopRecording()
{
    // Atomically take ownership; dtor may race us, whoever wins finalizes.
    void *h = m_handle.exchange(nullptr);
    if (!h) {
        return;
    }

    m_timer->stop();

    // The use-case stop adds the file to history on success (we set
    // add_to_history_on_stop in startRecording). Qt still validates the file
    // afterward so the user sees a clear error if disk-full / truncation
    // produced an unusable output.
    int rc = snapforge_record_stop(h);
    snapforge_record_free_handle(h);

    if (rc != 0) {
        QString detail = QStringLiteral("Recording failed during finalization");
        if (char *errMsg = snapforge_app_last_error()) {
            detail = QStringLiteral("Recording failed: %1").arg(QString::fromUtf8(errMsg));
            snapforge_free_string(errMsg);
        }
        qCritical("Recording finalize failed: %s", qUtf8Printable(detail));
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

    qInfo("Recording stopped: %s (%lld bytes)",
          qUtf8Printable(m_outputPath), (long long)fi.size());
    emit recordingStopped(m_outputPath);
}
