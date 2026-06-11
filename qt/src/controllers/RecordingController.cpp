#include "RecordingController.h"

#include "ClickIndicatorOverlay.h"
#include "ClickTap.h"
#include "PreferencesWindow.h"
#include "RecordingManager.h"
#include "TrayIcon.h"

#include <QApplication>
#include <QClipboard>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QMessageBox>
#include <QMimeData>
#include <QSystemTrayIcon>
#include <QTimer>
#include <QUrl>

RecordingController::RecordingController(RecordingManager      *recording,
                                         TrayIcon              *tray,
                                         ClickIndicatorOverlay *clickOverlay,
                                         ClickTap              *clickTap,
                                         PreferencesWindow     *prefs,
                                         QObject               *parent)
    : QObject(parent),
      m_recording(recording),
      m_tray(tray),
      m_clickOverlay(clickOverlay),
      m_clickTap(clickTap),
      m_prefs(prefs) {
    QObject::connect(m_recording, &RecordingManager::recordingStarted,
                     this, &RecordingController::onStarted);
    QObject::connect(m_recording, &RecordingManager::recordingStopped,
                     this, &RecordingController::onStopped);
    QObject::connect(m_recording, &RecordingManager::recordingPaused,
                     this, &RecordingController::onPaused);
    QObject::connect(m_recording, &RecordingManager::recordingResumed,
                     this, &RecordingController::onResumed);
    QObject::connect(m_recording, &RecordingManager::recordingError,
                     this, &RecordingController::onError);
    QObject::connect(m_recording, &RecordingManager::recordingWarning,
                     this, &RecordingController::onWarning);
    QObject::connect(m_recording, &RecordingManager::elapsedChanged,
                     this, &RecordingController::onElapsedChanged);
}

void RecordingController::onStarted(const QString & /*path*/) {
    m_tray->enterRecordingState(/*paused=*/false);

    if (m_prefs && m_prefs->showClicksEnabled()) {
        m_clickOverlay->showOverlay();
        if (m_clickTap && !m_clickTap->start()) {
            m_tray->showMessage(
                "Snapforge — Click indicator unavailable",
                "Grant Input Monitoring permission in System Settings → "
                "Privacy & Security to show clicks in recordings.",
                QSystemTrayIcon::Warning, 5000);
            m_clickOverlay->hideOverlay();
        }
    }
}

void RecordingController::onStopped(const QString &path) {
    m_tray->leaveRecordingState();

    if (m_clickTap) m_clickTap->stop();
    m_clickOverlay->hideOverlay();

    // Copy the finished recording file to the clipboard as a file URL so the
    // user can paste it into Finder, Messages, Slack, etc.
    if (!path.isEmpty() && QFile::exists(path)) {
        auto *mime = new QMimeData();
        mime->setUrls({ QUrl::fromLocalFile(path) });
        // Also include the path as plain text for apps that don't take URLs.
        mime->setText(path);
        QGuiApplication::clipboard()->setMimeData(mime);
        m_tray->showMessage("Snapforge — Recording saved",
                            "Copied to clipboard: " + QFileInfo(path).fileName(),
                            QSystemTrayIcon::Information, 3000);
    }
}

void RecordingController::onPaused() {
    m_tray->setPaused(true);
}

void RecordingController::onResumed() {
    m_tray->setPaused(false);
}

void RecordingController::onError(const QString &message) {
    qWarning("Recording error: %s", qPrintable(message));
    // Fatal (start/stop failure): reset the tray back to idle — leaving the
    // recording menu visible after a start-failure makes Stop/Pause clickable
    // for a recording that never began, which then no-ops confusingly.
    m_tray->leaveRecordingState();
    if (m_clickTap) m_clickTap->stop();
    m_clickOverlay->hideOverlay();
    m_tray->showMessage("Snapforge — Recording Failed", message,
                        QSystemTrayIcon::Warning, 5000);
    // Defer the modal so the recordingError signal completes its delivery
    // first; opening a blocking dialog inside the slot can re-enter the event
    // loop while RecordingManager is mid-cleanup.
    QTimer::singleShot(0, qApp, [message]() {
        QMessageBox box;
        box.setIcon(QMessageBox::Warning);
        box.setWindowTitle(QStringLiteral("Snapforge — Recording Failed"));
        box.setText(message);
        box.setInformativeText(QStringLiteral(
            "Common causes: Screen Recording permission not granted, "
            "ffmpeg missing from PATH, or selected output folder not writable. "
            "Open Preferences → Permissions to check, or relaunch Snapforge "
            "from a terminal to see the underlying error."));
        box.setStandardButtons(QMessageBox::Ok);
        box.exec();
    });
}

void RecordingController::onWarning(const QString &message) {
    // Non-fatal (pause/resume failure, incl. the auto-pause from
    // WorkspaceSleepObserver): the recording is still writing to disk, so
    // keep the tray in its recording state — resetting to idle here would
    // show idle while recording continues and hide the stop control. Just
    // log and notify.
    qWarning("Recording warning: %s", qPrintable(message));
    m_tray->showMessage("Snapforge — Recording", message,
                        QSystemTrayIcon::Warning, 5000);
}

void RecordingController::onElapsedChanged(int seconds) {
    if (m_recording->isRecording() || m_recording->isPaused()) {
        m_tray->updateElapsed(seconds);
    }
}
