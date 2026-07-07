#include "RecordingManager.h"

#include <QDateTime>
#include <QDebug>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QPointer>
#include <future>
#include <chrono>
#include <cstdlib>
#include <memory>
#include <thread>

#include "SnapforgeClient.h"

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
    // An interactive stopRecording() may still be finalizing on its worker.
    // Wait for it under the same 30s deadline as the in-place stop below —
    // letting it run detached past main() would have it calling into Rust
    // statics during process teardown (the exact UB documented below).
    if (m_stopWorker.joinable()) {
        if (m_stopDone.wait_for(std::chrono::seconds(30)) == std::future_status::ready) {
            m_stopWorker.join();
        } else {
            qWarning("RecordingManager: in-flight stop did not finalize within 30s; "
                     "aborting via _Exit to avoid FFI use-after-teardown");
            m_stopWorker.detach();
            std::_Exit(1);
        }
    }

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
        sf::RecHandle h{handle};
        sf::recordStop(h);
        sf::recordFreeHandle(h);
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
    if (!sf::recordPause(sf::RecHandle{h})) {
        // Non-fatal: the handle stays alive and the recording continues, so
        // surface a warning instead of recordingError (which resets the UI
        // to idle and hides the stop control while we're still recording).
        emit recordingWarning(QStringLiteral("Failed to pause recording"));
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
    if (!sf::recordResume(sf::RecHandle{h})) {
        // Non-fatal: still paused but the recording session is alive — see
        // pauseRecording above.
        emit recordingWarning(QStringLiteral("Failed to resume recording"));
        return;
    }
    m_elapsed.start();
    m_paused = false;
    // Any successful resume (manual or wake-triggered) disarms the sleep
    // handler's auto-resume.
    m_autoPausedBySleep = false;
    emit recordingResumed();
}

void RecordingManager::handleSystemSleep()
{
    // Pause only an actively-running recording; a manual pause stays manual
    // (flag untouched) so the wake handler won't resume it.
    if (!isRecording() || m_paused) return;
    pauseRecording();
    // Arm auto-resume only if the pause actually took effect.
    m_autoPausedBySleep = m_paused;
}

void RecordingManager::handleSystemWake()
{
    if (!m_autoPausedBySleep) return;
    m_autoPausedBySleep = false;
    resumeRecording();
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
    m_prefShowClicks = false;

    QByteArray cfg = sf::configLoadJson();
    if (!cfg.isEmpty()) {
        QJsonDocument doc = QJsonDocument::fromJson(cfg);
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
        m_prefShowClicks = rec.value(QStringLiteral("show_clicks")).toBool();
    }
    m_prefsLoaded = true;
}

void RecordingManager::startRecording(int display, QRect region, QString outputDir)
{
    if (isRecording() || m_stopping) {
        // Non-fatal: the current recording / finalization continues untouched.
        // recordingError here would make RecordingController leave recording
        // state and hide the stop control while the real recording runs on.
        // ponytail: a start during finalization is rejected, not queued.
        emit recordingWarning(m_stopping
            ? QStringLiteral("Still finalizing the previous recording")
            : QStringLiteral("Already recording"));
        return;
    }

    loadPrefsIfNeeded();
    QString fmt     = m_prefFormat;
    int     fps     = m_prefFps;
    QString quality = m_prefQuality;

    // Generate timestamped output path with extension matching selected format.
    QString timestamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd_HH-mm-ss"));
    m_outputPath = outputDir + QStringLiteral("/recording_") + timestamp + QStringLiteral(".") + fmt;

    // Build the typed request; the client serializes the JSON internally.
    sf::RecordReq req;
    req.display    = static_cast<uint32_t>(display);
    req.outputPath = m_outputPath;
    req.format     = fmt;
    req.fps        = static_cast<uint32_t>(fps);
    req.quality    = quality;
    req.showClicks = m_prefShowClicks;
    // ffmpegPath left as nullopt -> the client emits a null ffmpeg_path, same
    // as the old QJsonValue::Null we passed.
    if (!region.isNull()) {
        req.region = sf::Region{
            region.x(),
            region.y(),
            static_cast<uint32_t>(region.width()),
            static_cast<uint32_t>(region.height()),
        };
    }
    // Use-case handles history indexing on successful stop. Qt still does its
    // own existence/incomplete-mp4 sanity checks below so the user gets a
    // specific error message instead of a silent skip.
    req.addToHistoryOnStop = true;

    sf::RecHandle newHandle = sf::recordStart(req);
    m_handle.store(newHandle.p);
    if (!newHandle.valid()) {
        QString detail = QStringLiteral("Failed to start recording");
        QString err = sf::lastError();
        if (!err.isEmpty()) {
            detail = QStringLiteral("Recording failed: %1").arg(err);
        }
        emit recordingError(detail);
        return;
    }

    m_accumulatedMs = 0;
    m_paused = false;
    m_autoPausedBySleep = false;
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
    m_paused = false;
    m_autoPausedBySleep = false;
    // Gate re-entry: the handle is already nulled so isRecording() is false,
    // but a new start must wait until the finalize completes (finishStop
    // clears this). startRecording rejects with a warning meanwhile.
    m_stopping = true;

    // Collect a previous worker whose result was already delivered (finishStop
    // joins too; this is belt-and-braces and returns immediately).
    if (m_stopWorker.joinable()) m_stopWorker.join();

    // recordStop blocks until ffmpeg finalizes the file, which can take
    // seconds on a long recording — run it off the GUI thread (same reason as
    // the dtor's worker) and marshal the outcome back. The dtor joins this
    // worker before the object dies, so `self` is always valid at invoke time;
    // the queued call itself is dropped by Qt if we're destroyed before it runs.
    auto donePromise = std::make_shared<std::promise<void>>();
    m_stopDone = donePromise->get_future();
    QPointer<RecordingManager> self(this);
    m_stopWorker = std::thread([self, h, donePromise]() {
        // The use-case stop adds the file to history on success (we set
        // add_to_history_on_stop in startRecording). finishStop still
        // validates the file so the user sees a clear error if disk-full /
        // truncation produced an unusable output.
        const bool ok = sf::recordStop(sf::RecHandle{h});
        // lastError is only meaningful right after the failing call — read it
        // here on the worker, not later on the GUI thread.
        const QString err = ok ? QString() : sf::lastError();
        sf::recordFreeHandle(sf::RecHandle{h});
        QMetaObject::invokeMethod(self.data(), [self, ok, err]() {
            if (!self) return;
            self->finishStop(ok, err);
        }, Qt::QueuedConnection);
        donePromise->set_value();
    });
}

// GUI-thread continuation of stopRecording once the worker's recordStop has
// returned. Emits the lifecycle signals and re-opens the start gate.
void RecordingManager::finishStop(bool ok, const QString &err)
{
    m_stopping = false;
    if (m_stopWorker.joinable()) m_stopWorker.join();

    if (!ok) {
        QString detail = QStringLiteral("Recording failed during finalization");
        if (!err.isEmpty()) {
            detail = QStringLiteral("Recording failed: %1").arg(err);
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
    if (sf::isIncompleteMp4(m_outputPath)) {
        emit recordingError(QStringLiteral("Recording output is corrupt (missing moov atom)"));
        return;
    }

    qInfo("Recording stopped: %s (%lld bytes)",
          qUtf8Printable(m_outputPath), (long long)fi.size());
    emit recordingStopped(m_outputPath);
}
