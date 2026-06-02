#include <QApplication>
#include <QSystemTrayIcon>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include <QIcon>
#include <QEvent>
#include <QJsonDocument>
#include <QJsonObject>
#include "OverlayWindow.h"
#include "RecordingManager.h"
#include "HistoryWindow.h"
#include "PreferencesWindow.h"
#include "ClickIndicatorOverlay.h"
#include "TrayIcon.h"
#include "RecordingController.h"
#include "ClickTap.h"
#include "Logger.h"
#include "Shortcuts.h"
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

static UInt32 hotkeyIdFor(const QString &actionKey) {
    if (actionKey == QLatin1String("screenshot")) return kHotkeyIDScreenshot;
    if (actionKey == QLatin1String("record"))     return kHotkeyIDRecord;
    if (actionKey == QLatin1String("history"))     return kHotkeyIDHistory;
    if (actionKey == QLatin1String("fullscreen"))  return kHotkeyIDFullscreen;
    return 0;
}

// Register every global chord from the shared config (shortcuts::chord), parsed
// into Carbon vk+mods. g_hotkeys[i] mirrors shortcuts::kGlobalActions[i] so
// unregisterGlobalHotkeys can release them all. An unparseable or in-use chord
// is logged and left null — the rest still register.
static void registerAllChords() {
    EventHotKeyID hotKeyID;
    hotKeyID.signature = 'SNPF';

    int i = 0;
    for (const auto &action : shortcuts::kGlobalActions) {
        const QString key = QString::fromLatin1(action.actionKey);
        const QString chord = shortcuts::chord(key);
        uint32_t vk = 0, mods = 0;
        hotKeyID.id = hotkeyIdFor(key);
        if (shortcuts::toCarbon(chord, &vk, &mods)) {
            const QByteArray name = QStringLiteral("%1 (%2)").arg(chord, key).toUtf8();
            registerHotkeyChecked(vk, mods, hotKeyID, &g_hotkeys[i], name.constData());
        } else {
            qWarning("Hotkey '%s' has unparseable chord \"%s\" — not registered",
                     action.actionKey, qPrintable(chord));
            g_hotkeys[i] = nullptr;
        }
        ++i;
    }
}

void registerGlobalHotkey() {
    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind  = kEventHotKeyPressed;

    // Install the dispatch handler exactly once; chords are (re)bound separately.
    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    registerAllChords();
}

void unregisterGlobalHotkeys() {
    for (auto &ref : g_hotkeys) {
        if (ref) {
            UnregisterEventHotKey(ref);
            ref = nullptr;
        }
    }
}

// Re-read chords from config and rebind. Called after Preferences saves new
// hotkeys so the live global shortcuts follow without an app restart. The
// dispatch handler installed in registerGlobalHotkey stays put.
void reloadGlobalHotkeys() {
    unregisterGlobalHotkeys();
    registerAllChords();
}
#endif

static PreferencesWindow *g_prefsRef = nullptr;

// Bridge Rust `tracing` records into the Qt Logger so Rust diagnostics land in
// the same rotating log file as the C++ side. Registered with
// snapforge_set_log_callback at startup. Fires from arbitrary Rust threads;
// Logger::log is mutex-guarded so this is safe to call off the main thread.
// Must not throw — keep it noexcept and trivial.
static void rustLogCallback(int level, const char *msg) noexcept {
    if (!msg) return;
    QtMsgType qtLevel;
    switch (level) {
        case SNAPFORGE_LOG_TRACE:
        case SNAPFORGE_LOG_DEBUG: qtLevel = QtDebugMsg;    break;
        case SNAPFORGE_LOG_INFO:  qtLevel = QtInfoMsg;     break;
        case SNAPFORGE_LOG_WARN:  qtLevel = QtWarningMsg;  break;
        case SNAPFORGE_LOG_ERROR: qtLevel = QtCriticalMsg; break;
        default:                  qtLevel = QtInfoMsg;     break;
    }
    Logger::instance()->log(qtLevel, QStringLiteral("rust"),
                            QString::fromUtf8(msg));
}

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
    // writable. Let snapforge_save_prerendered attempt the write and surface
    // the real errno via the tray if it fails.
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

    // Use-case FFI: snapforge_save_prerendered handles encode+write,
    // (optional) clipboard, and (optional) history indexing in one call.
    // We pass the Qt-composited RGBA bytes verbatim — same input the
    // deprecated snapforge_save_image used to take.
    static const char *kFmtNames[] = { "png", "jpg", "webp" };
    QJsonObject req;
    req[QStringLiteral("output_path")] = path;
    req[QStringLiteral("format")] = QString::fromLatin1(kFmtNames[qBound(0, fmt, 2)]);
    req[QStringLiteral("quality")] = quality;
    req[QStringLiteral("copy_to_clipboard")] = false; // copyImage handles that separately
    req[QStringLiteral("add_to_history")] = true;
    QByteArray reqBytes = QJsonDocument(req).toJson(QJsonDocument::Compact);

    char *resJson = snapforge_save_prerendered(rgba.constBits(),
                                               static_cast<size_t>(rgba.sizeInBytes()),
                                               rgba.width(), rgba.height(),
                                               reqBytes.constData());
    if (resJson) {
        snapforge_free_string(resJson);
        qDebug("Saved: %s", qPrintable(path));

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
        QString errDetail;
        if (char *err = snapforge_app_last_error()) {
            errDetail = QString::fromUtf8(err);
            snapforge_free_string(err);
        }
        auto *tray = qobject_cast<QSystemTrayIcon *>(
            qApp->property("systemTray").value<QObject *>());
        if (tray) {
            tray->showMessage("Snapforge — Save failed",
                              "Could not write " + filename + " (disk full or permission denied?)",
                              QSystemTrayIcon::Warning, 5000);
        }
        qWarning("Save failed: snapforge_save_prerendered returned NULL for %s: %s",
                 qPrintable(path), qPrintable(errDetail));
    }
}

static void copyImage(const QImage &img) {
    if (img.isNull()) return;
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);

    // Clipboard-only: omit output_path so the use case skips encode + write
    // + history and only touches NSPasteboard.
    QJsonObject req;
    req[QStringLiteral("copy_to_clipboard")] = true;
    QByteArray reqBytes = QJsonDocument(req).toJson(QJsonDocument::Compact);
    char *resJson = snapforge_save_prerendered(rgba.constBits(),
                                               static_cast<size_t>(rgba.sizeInBytes()),
                                               rgba.width(), rgba.height(),
                                               reqBytes.constData());
    if (resJson) {
        snapforge_free_string(resJson);
        qDebug("Copied to clipboard");
    } else {
        if (char *err = snapforge_app_last_error()) {
            qWarning("Clipboard copy failed: %s", err);
            snapforge_free_string(err);
        } else {
            qWarning("Clipboard copy failed");
        }
    }
}

int main(int argc, char *argv[]) {
    // CRITICAL: Suppress Qt's internal [NSApp activateIgnoringOtherApps:YES]
    // in QCocoaWindow::raise(). Without this, raise() triggers a Space switch.
    qputenv("QT_MAC_SET_RAISE_PROCESS", "0");

    // Route qDebug/qWarning/qInfo/qCritical to the on-disk log + ring buffer
    // before any other init so startup messages get captured.
    Logger::install();

    // Route Rust `tracing` diagnostics into the same log file. Done right after
    // Logger::install so any Rust call below (permission check, etc.) is logged.
    snapforge_set_log_callback(rustLogCallback);

    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
#ifdef SNAPFORGE_VERSION
    app.setApplicationVersion(QStringLiteral(SNAPFORGE_VERSION));
#endif
    Logger::instance()->logBanner();
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
    // ClickTap wraps the snapforge_clicks_* use-case FFI — the platform tap
    // lives in Rust now, so this works on every platform Snapforge ships.
    ClickIndicatorOverlay clickOverlay;
    ClickTap clickTap;
    QObject::connect(&clickTap, &ClickTap::clicked,
                     &clickOverlay, &ClickIndicatorOverlay::addRipple);

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

    // A hotkey rebind in Preferences must reach all three encoders of a chord:
    // the live Carbon hotkeys, the tray menu glyphs, and (already handled) the
    // prefs badges. Re-register + rebuild the menu on save so nothing drifts.
    QObject::connect(&prefs, &PreferencesWindow::configSaved, &tray, &TrayIcon::refreshMenu);
#ifdef Q_OS_MAC
    QObject::connect(&prefs, &PreferencesWindow::configSaved, &tray, []() { reloadGlobalHotkeys(); });
#endif

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

    // Wire all RecordingManager lifecycle signals → tray feedback + click
    // overlay/tap toggling + clipboard copy on stop + error modal.
    RecordingController recordingController(&recording,
                                            &tray,
                                            &clickOverlay,
                                            &clickTap,
                                            &prefs,
                                            &app);
    Q_UNUSED(recordingController);

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
