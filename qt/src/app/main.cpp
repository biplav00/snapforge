#include <QApplication>
#include <QSystemTrayIcon>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include <QIcon>
#include <QClipboard>
#include <QMimeData>
#include <QUrl>
#include <QEvent>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QMessageBox>
#include "OverlayWindow.h"
#include "RecordingManager.h"
#include "HistoryWindow.h"
#include "PreferencesWindow.h"
#include "ClickIndicatorOverlay.h"
#include "TrayIcon.h"
#ifdef Q_OS_MACOS
#include "ClickEventTap.h"
#endif
#include "Logger.h"
#include "snapforge_ffi.h"
#ifdef Q_OS_MAC
#include "WorkspaceSleepObserver.h"
#include "SpaceChangeObserver.h"
#endif

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>
#include <objc/runtime.h>
#include <objc/message.h>
#include <array>

// macOS global hotkey callback
static OverlayWindow     *g_overlay   = nullptr;
static RecordingManager  *g_recording = nullptr;
static HistoryWindow     *g_history   = nullptr;

// Hotkey IDs
static const UInt32 kHotkeyIDScreenshot = 1;
static const UInt32 kHotkeyIDRecord     = 2;
static const UInt32 kHotkeyIDHistory    = 3;
static const UInt32 kHotkeyIDFullscreen = 4;

// H6: track every registered hotkey so we can unregister them on quit.
// Previously we only kept the last one, so two were leaked on app exit.
static std::array<EventHotKeyRef, 4> g_hotkeys = { nullptr, nullptr, nullptr, nullptr };

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef event, void *) {
    EventHotKeyID firedID;
    GetEventParameter(event, kEventParamDirectObject,
                      typeEventHotKeyID, nullptr,
                      sizeof(firedID), nullptr, &firedID);

    // Fix #14: ignore any hotkey that would re-enter the overlay while it's
    // already doing real work (drawing, region committed, annotating, or
    // mid-capture). Visible-but-idle is treated as recoverable: AppKit can
    // hide an NSPanel out from under Qt on deactivation, leaving isVisible()
    // desynced. isBusy() consults real state instead of bare visibility.
    const bool overlayBusy = g_overlay && g_overlay->isBusy();
    qDebug("hotkey id=%u overlayExists=%d overlayVisible=%d overlayBusy=%d overlayMinimized=%d",
           (unsigned)firedID.id,
           g_overlay != nullptr,
           g_overlay && g_overlay->isVisible(),
           overlayBusy,
           g_overlay && g_overlay->isMinimized());

    // Idle-hide: if overlay is visible but not busy, force a clean reset
    // before re-activating so we don't stack a fresh activate() on top of a
    // stale shown-but-desynced window.
    auto resetIfStale = []() {
        if (g_overlay && g_overlay->isVisible()) g_overlay->hide();
    };

    auto notifyBusy = []() {
        // Give the user some feedback so the hotkey doesn't feel dead. The
        // record-toggle path intentionally bypasses this — stopping an
        // in-progress recording is the whole point.
        QApplication::beep();
        auto *tray = qobject_cast<QSystemTrayIcon *>(
            qApp->property("systemTray").value<QObject *>());
        if (tray) {
            tray->showMessage("Snapforge",
                              "Capture in progress",
                              QSystemTrayIcon::Information, 2000);
        }
    };

    switch (firedID.id) {
    case kHotkeyIDScreenshot:
        if (g_overlay) {
            if (overlayBusy) notifyBusy();
            else {
                resetIfStale();
                QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
            }
        }
        break;

    case kHotkeyIDFullscreen:
        if (g_overlay) {
            if (overlayBusy) notifyBusy();
            else {
                resetIfStale();
                QTimer::singleShot(0, g_overlay, &OverlayWindow::activateFullscreen);
            }
        }
        break;

    case kHotkeyIDRecord:
        if (g_recording && g_overlay) {
            if (g_recording->isRecording()) {
                QTimer::singleShot(0, g_recording, &RecordingManager::stopRecording);
            } else if (overlayBusy) {
                notifyBusy();
            } else {
                resetIfStale();
                QTimer::singleShot(0, g_overlay, &OverlayWindow::activateForRecording);
            }
        }
        break;

    case kHotkeyIDHistory:
        if (g_history) {
            if (overlayBusy) {
                notifyBusy();
            } else {
                QTimer::singleShot(0, []() {
                    g_history->refreshHistory();
                    g_history->show();
                    g_history->raise();
                    g_history->activateWindow();
                });
            }
        }
        break;

    default:
        break;
    }

    return noErr;
}

// Wrap RegisterEventHotKey so a failed registration (e.g. another app already
// owns the chord) is logged instead of silently swallowed. `name` is shown so
// the user can correlate the warning with the action that won't fire.
static void registerHotkeyChecked(unsigned int virtualKey,
                                  unsigned int modifiers,
                                  EventHotKeyID hotKeyID,
                                  EventHotKeyRef *outRef,
                                  const char *name) {
    OSStatus status = RegisterEventHotKey(virtualKey, modifiers, hotKeyID,
                                          GetApplicationEventTarget(), 0, outRef);
    if (status != noErr || *outRef == nullptr) {
        qWarning("RegisterEventHotKey failed for %s (status=%d) — chord may be in use by another app",
                 name, static_cast<int>(status));
        *outRef = nullptr;
    }
}

void registerGlobalHotkey() {
    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind  = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    EventHotKeyID  hotKeyID;
    hotKeyID.signature = 'SNPF';

    // Cmd+Shift+S — Screenshot (kVK_ANSI_S = 0x01)
    hotKeyID.id = kHotkeyIDScreenshot;
    registerHotkeyChecked(0x01, cmdKey | shiftKey, hotKeyID, &g_hotkeys[0], "Cmd+Shift+S (screenshot)");

    // Cmd+Shift+R — Record (kVK_ANSI_R = 0x0F)
    hotKeyID.id = kHotkeyIDRecord;
    registerHotkeyChecked(0x0F, cmdKey | shiftKey, hotKeyID, &g_hotkeys[1], "Cmd+Shift+R (record)");

    // Cmd+Shift+H — History (kVK_ANSI_H = 0x04)
    hotKeyID.id = kHotkeyIDHistory;
    registerHotkeyChecked(0x04, cmdKey | shiftKey, hotKeyID, &g_hotkeys[2], "Cmd+Shift+H (history)");

    // Cmd+Shift+F — Fullscreen capture (kVK_ANSI_F = 0x03)
    hotKeyID.id = kHotkeyIDFullscreen;
    registerHotkeyChecked(0x03, cmdKey | shiftKey, hotKeyID, &g_hotkeys[3], "Cmd+Shift+F (fullscreen)");
}

void unregisterGlobalHotkeys() {
    for (auto &ref : g_hotkeys) {
        if (ref) {
            UnregisterEventHotKey(ref);
            ref = nullptr;
        }
    }
}
#endif

static PreferencesWindow *g_prefsRef = nullptr;

static QString buildFilename(const QString &pattern, int fmt) {
    QString timestamp = QDateTime::currentDateTime().toString("yyyy-MM-dd_HH-mm-ss");
    const char *extensions[] = { "png", "jpg", "webp" };
    QString ext = extensions[qBound(0, fmt, 2)];

    if (pattern.isEmpty()) {
        return QStringLiteral("screenshot_%1.%2").arg(timestamp, ext);
    }

    // Replace {date}, {time}, {datetime} tokens
    QString name = pattern;
    name.replace("{date}", QDateTime::currentDateTime().toString("yyyy-MM-dd"));
    name.replace("{time}", QDateTime::currentDateTime().toString("HH-mm-ss"));
    name.replace("{datetime}", timestamp);

    // Ensure correct extension
    if (!name.endsWith("." + ext)) {
        // Remove any existing extension from pattern
        int dot = name.lastIndexOf('.');
        if (dot > 0) name.truncate(dot);
        name += "." + ext;
    }
    return name;
}

static void saveImage(const QImage &img) {
    if (img.isNull()) return;
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);

    // Read preferences
    int fmt = g_prefsRef ? g_prefsRef->screenshotFormat() : 0;
    int quality = g_prefsRef ? g_prefsRef->jpgQuality() : 90;
    QString pattern = g_prefsRef ? g_prefsRef->filenamePattern() : QString();

    // Save directory
    QString dir;
    if (g_prefsRef && !g_prefsRef->saveDirectory().isEmpty()) {
        dir = g_prefsRef->saveDirectory();
    } else {
        char *saveDir = snapforge_default_save_path();
        dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Pictures/Snapforge";
        if (saveDir) snapforge_free_string(saveDir);
    }

    // Fix #16: if the directory doesn't exist, try to create it. We skip the
    // QFileInfo::isWritable pre-check — it's unreliable on macOS under
    // sandboxing / TCC and can return false for paths that are in fact
    // writable. Let snapforge_save_image attempt the write and surface the
    // real errno via the tray if it fails.
    QDir d(dir);
    if (!d.exists()) {
        if (!QDir().mkpath(dir)) {
            auto *tray = qobject_cast<QSystemTrayIcon *>(
                qApp->property("systemTray").value<QObject *>());
            if (tray) {
                tray->showMessage("Snapforge — Save failed",
                                  "Could not create save directory: " + dir,
                                  QSystemTrayIcon::Warning, 5000);
            }
            qWarning("Save failed: cannot create %s", qPrintable(dir));
            return;
        }
    }
    QString filename = buildFilename(pattern, fmt);
    QString path = dir + "/" + filename;

    int result = snapforge_save_image(rgba.constBits(), rgba.width(), rgba.height(),
                                      path.toUtf8().constData(), fmt, static_cast<uint8_t>(quality));
    if (result == 0) {
        qDebug("Saved: %s", qPrintable(path));
        snapforge_history_add(path.toUtf8().constData());

        // Show notification if enabled
        if (g_prefsRef && g_prefsRef->showNotifEnabled()) {
            auto *tray = qobject_cast<QSystemTrayIcon *>(
                qApp->property("systemTray").value<QObject *>());
            if (tray) {
                tray->showMessage("Snapforge", "Screenshot saved: " + filename,
                                  QSystemTrayIcon::Information, 2000);
            }
        }
    } else {
        auto *tray = qobject_cast<QSystemTrayIcon *>(
            qApp->property("systemTray").value<QObject *>());
        if (tray) {
            tray->showMessage("Snapforge — Save failed",
                              "Could not write " + filename + " (disk full or permission denied?)",
                              QSystemTrayIcon::Warning, 5000);
        }
        qWarning("Save failed: snapforge_save_image returned %d for %s", result, qPrintable(path));
    }
}

static void copyImage(const QImage &img) {
    if (img.isNull()) return;
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);
    snapforge_copy_to_clipboard(rgba.constBits(), rgba.width(), rgba.height());
    qDebug("Copied to clipboard");
}

int main(int argc, char *argv[]) {
    // CRITICAL: Suppress Qt's internal [NSApp activateIgnoringOtherApps:YES]
    // in QCocoaWindow::raise(). Without this, raise() triggers a Space switch.
    qputenv("QT_MAC_SET_RAISE_PROCESS", "0");

    // Route qDebug/qWarning/qInfo/qCritical to the on-disk log + ring buffer
    // before any other init so startup messages get captured.
    Logger::install();

    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
    qInfo("Snapforge starting");
    app.setQuitOnLastWindowClosed(false);

    // App-wide icon used by Dock, task switcher, and any generic windows.
    // Tray uses its own smaller variant further down.
    {
        QIcon appIcon;
        appIcon.addFile(":/icons/app-512.png", QSize(512, 512));
        appIcon.addFile(":/icons/tray@2x.png", QSize(256, 256));
        appIcon.addFile(":/icons/tray.png", QSize(128, 128));
        if (!appIcon.isNull()) {
            QApplication::setWindowIcon(appIcon);
        }
    }

#ifdef Q_OS_MAC
    // Set activation policy to Accessory — this is the KEY to preventing
    // Space switches. An accessory app can show windows over fullscreen
    // apps without macOS pulling the user to another Space.
    // This is equivalent to Electron's app.dock.hide() which is how
    // CleanShot X, Shottr, and similar tools work.
    {
        id nsApp = (id)objc_getClass("NSApplication");
        id sharedApp = ((id (*)(id, SEL))objc_msgSend)(nsApp, sel_registerName("sharedApplication"));
        // NSApplicationActivationPolicyAccessory = 1
        ((void (*)(id, SEL, long))objc_msgSend)(sharedApp, sel_registerName("setActivationPolicy:"), 1);
    }
#endif

    // Request permission if needed
    if (!snapforge_has_permission()) {
        snapforge_request_permission();
    }

    // Pre-create overlay and warm up the window server registration
    // Use zero-opacity show to avoid visible flash
    OverlayWindow overlay;
    g_overlay = &overlay;
    // No pre-warm needed — opaque overlay with screenshot background
    // eliminates visible window transition

    // Handle screenshot save (+ auto-copy if enabled)
    QObject::connect(&overlay, &OverlayWindow::screenshotReady,
                     [](QImage composited, int /*w*/, int /*h*/) {
        saveImage(composited);
        if (g_prefsRef && g_prefsRef->autoCopyEnabled()) {
            copyImage(composited);
        }
    });

    // Handle clipboard copy
    QObject::connect(&overlay, &OverlayWindow::clipboardReady,
                     [](QImage composited, int /*w*/, int /*h*/) {
        copyImage(composited);
    });

    // Create manager/window instances
    RecordingManager recording;

#ifdef Q_OS_MAC
    // Fix #10: auto-pause/resume around system sleep.
    WorkspaceSleepObserver sleepObserver(&recording);
    Q_UNUSED(sleepObserver);
    // Re-activate overlay when user returns from another Space / fullscreen
    // app — otherwise key-window state is lost and shortcuts go dead.
    SpaceChangeObserver spaceObserver(&overlay);
    Q_UNUSED(spaceObserver);
#endif

    // Connect recording requested from overlay
    QObject::connect(&overlay, &OverlayWindow::recordingRequested,
                     [&](int display, QRect region) {
        char *saveDir = snapforge_default_save_path();
        QString dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Movies/Snapforge";
        if (saveDir) snapforge_free_string(saveDir);
        recording.startRecording(display, region, dir);
    });
    g_recording = &recording;

    HistoryWindow history;
    g_history = &history;

    PreferencesWindow prefs;
    g_prefsRef = &prefs;

    // Fix #20: re-apply theme whenever the system palette changes. The
    // filter fires for any QApplication palette change (which QPA triggers
    // on macOS dark mode flips). "Auto" follows; explicit Light/Dark ignore.
    class PaletteFilter : public QObject {
    public:
        PreferencesWindow *prefs;
        explicit PaletteFilter(PreferencesWindow *p, QObject *parent)
            : QObject(parent), prefs(p) {}
        bool eventFilter(QObject *obj, QEvent *event) override {
            if (event->type() == QEvent::ApplicationPaletteChange && prefs) {
                prefs->applyTheme();
            }
            return QObject::eventFilter(obj, event);
        }
    };
    auto *paletteFilter = new PaletteFilter(&prefs, &app);
    app.installEventFilter(paletteFilter);

    // Click visualizer: ripple overlay + global mouse-down tap.
    // Tap is only started while recording AND the pref is on. Permission
    // failure surfaces as a tray banner; the recording itself continues.
    ClickIndicatorOverlay clickOverlay;
#ifdef Q_OS_MACOS
    ClickEventTap clickTap;
    QObject::connect(&clickTap, &ClickEventTap::clicked,
                     &clickOverlay, &ClickIndicatorOverlay::addRipple);
#endif

    // Sync prefs → overlay
    auto syncPrefsToOverlay = [&]() {
        overlay.setRememberRegion(prefs.rememberRegionEnabled());
    };
    QObject::connect(&prefs, &PreferencesWindow::configSaved, syncPrefsToOverlay);
    // M13: when prefs change, re-read recording config into RecordingManager.
    QObject::connect(&prefs, &PreferencesWindow::configSaved,
                     &recording, &RecordingManager::reloadPrefs);
    // Initial sync after prefs load (deferred so loadConfig runs first)
    QTimer::singleShot(0, syncPrefsToOverlay);

    // Tray icon (idle/pill rendering, menu builders, pulse timer) lives in
    // its own class. We connect its action* signals to the same callers the
    // inline menu lambdas used to invoke directly.
    TrayIcon tray;
    tray.initialize();

    QObject::connect(&tray, &TrayIcon::actionScreenshot,
                     &overlay, &OverlayWindow::activate);
    QObject::connect(&tray, &TrayIcon::actionFullscreen,
                     &overlay, &OverlayWindow::activateFullscreen);
    QObject::connect(&tray, &TrayIcon::actionRecordToggle, [&]() {
        if (recording.isRecording()) {
            recording.stopRecording();
        } else {
            overlay.activateForRecording();
        }
    });
    QObject::connect(&tray, &TrayIcon::actionHistory, [&]() {
        history.refreshHistory();
        history.show();
        history.raise();
        history.activateWindow();
    });
    QObject::connect(&tray, &TrayIcon::actionPreferences, [&]() {
        prefs.show();
        prefs.raise();
        prefs.activateWindow();
    });
    QObject::connect(&tray, &TrayIcon::actionQuit, &app, &QApplication::quit);
    QObject::connect(&tray, &TrayIcon::actionPauseRecording,
                     &recording, &RecordingManager::pauseRecording);
    QObject::connect(&tray, &TrayIcon::actionResumeRecording,
                     &recording, &RecordingManager::resumeRecording);
    QObject::connect(&tray, &TrayIcon::actionStopRecording,
                     &recording, &RecordingManager::stopRecording);

    // Q2: surface invalid-selection feedback (too small, multi-display) via
    // the tray instead of silently dropping.
    QObject::connect(&overlay, &OverlayWindow::regionInvalid,
                     [&tray](const QString &reason) {
        tray.showMessage("Snapforge — Selection invalid",
                         reason,
                         QSystemTrayIcon::Information, 3000);
    });

    // Update the timer text every second when elapsedChanged fires.
    QObject::connect(&recording, &RecordingManager::elapsedChanged,
                     [&tray, &recording](int secs) {
        if (recording.isRecording() || recording.isPaused()) {
            tray.updateElapsed(secs);
        }
    });

    // Pause/Resume: ask the tray to swap pill animation + menu layout.
    QObject::connect(&recording, &RecordingManager::recordingPaused, [&tray]() {
        tray.setPaused(true);
    });
    QObject::connect(&recording, &RecordingManager::recordingResumed, [&tray]() {
        tray.setPaused(false);
    });

    // On start: swap the tray icon to the recording pill and kick off the pulse.
    QObject::connect(&recording, &RecordingManager::recordingStarted,
                     [&](const QString &/*path*/) {
        tray.enterRecordingState(/*paused=*/false);

        if (prefs.showClicksEnabled()) {
            clickOverlay.showOverlay();
#ifdef Q_OS_MACOS
            if (!clickTap.start()) {
                tray.showMessage(
                    "Snapforge — Click indicator unavailable",
                    "Grant Input Monitoring permission in System Settings → "
                    "Privacy & Security to show clicks in recordings.",
                    QSystemTrayIcon::Warning, 5000);
                clickOverlay.hideOverlay();
            }
#endif
        }
    });

    QObject::connect(&recording, &RecordingManager::recordingStopped,
                     [&](const QString &path) {
        tray.leaveRecordingState();

#ifdef Q_OS_MACOS
        clickTap.stop();
#endif
        clickOverlay.hideOverlay();

        // Copy the finished recording file to the clipboard as a file URL so
        // the user can paste it into Finder, Messages, Slack, etc.
        if (!path.isEmpty() && QFile::exists(path)) {
            auto *mime = new QMimeData();
            mime->setUrls({ QUrl::fromLocalFile(path) });
            // Also include the path as plain text for apps that don't take URLs.
            mime->setText(path);
            QGuiApplication::clipboard()->setMimeData(mime);
            tray.showMessage("Snapforge — Recording saved",
                             "Copied to clipboard: " + QFileInfo(path).fileName(),
                             QSystemTrayIcon::Information, 3000);
        }
    });

    // Surface recording failures to the user instead of silently dropping them.
    // Tray banners get suppressed by Notification Center for unsigned bundles,
    // so we also pop a modal warning as the source of truth — without it the
    // failure looks identical to "nothing happened" and the user can't act.
    QObject::connect(&recording, &RecordingManager::recordingError,
                     [&](const QString &message) {
        qWarning("Recording error: %s", qPrintable(message));
        // Reset the tray back to idle — leaving the recording menu visible
        // after a start-failure makes Stop/Pause clickable for a recording
        // that never began, which then no-ops confusingly.
        tray.leaveRecordingState();
#ifdef Q_OS_MACOS
        clickTap.stop();
#endif
        clickOverlay.hideOverlay();
        tray.showMessage("Snapforge — Recording Failed", message,
                         QSystemTrayIcon::Warning, 5000);
        // Defer the modal so the recordingError signal completes its delivery
        // first; opening a blocking dialog inside the slot can re-enter the
        // event loop while RecordingManager is mid-cleanup.
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
    });

#ifdef Q_OS_MAC
    registerGlobalHotkey();
    QObject::connect(&app, &QCoreApplication::aboutToQuit, []() {
        // Zero globals BEFORE tearing down the Carbon hotkey registration so
        // any handler invocation racing with shutdown sees null pointers
        // instead of stack objects about to be unwound. The hotkey handler
        // null-checks every global before use.
        g_overlay = nullptr;
        g_recording = nullptr;
        g_history = nullptr;
        g_prefsRef = nullptr;
        unregisterGlobalHotkeys();
    });
#endif

    int rc = app.exec();

    // Non-mac platforms (and as belt-and-braces for mac if aboutToQuit
    // somehow didn't fire) — globals are raw pointers to stack objects that
    // are about to unwind; null them before return.
    g_prefsRef = nullptr;
#ifndef Q_OS_MAC
    // No-op on non-mac builds: the mac-only globals don't exist here.
#else
    g_overlay = nullptr;
    g_recording = nullptr;
    g_history = nullptr;
#endif

    return rc;
}
