#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include "OverlayWindow.h"
#include "RecordingManager.h"
#include "HistoryWindow.h"
#include "PreferencesWindow.h"
#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>
#include <objc/runtime.h>
#include <objc/message.h>

// macOS global hotkey callback
static OverlayWindow     *g_overlay   = nullptr;
static RecordingManager  *g_recording = nullptr;
static HistoryWindow     *g_history   = nullptr;
static PreferencesWindow *g_prefs     = nullptr;

// Hotkey IDs
static const UInt32 kHotkeyIDScreenshot = 1;
static const UInt32 kHotkeyIDRecord     = 2;
static const UInt32 kHotkeyIDHistory    = 3;

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef event, void *) {
    EventHotKeyID firedID;
    GetEventParameter(event, kEventParamDirectObject,
                      typeEventHotKeyID, nullptr,
                      sizeof(firedID), nullptr, &firedID);

    switch (firedID.id) {
    case kHotkeyIDScreenshot:
        if (g_overlay)
            QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
        break;

    case kHotkeyIDRecord:
        if (g_recording && g_overlay) {
            if (g_recording->isRecording()) {
                QTimer::singleShot(0, g_recording, &RecordingManager::stopRecording);
            } else {
                QTimer::singleShot(0, g_overlay, &OverlayWindow::activateForRecording);
            }
        }
        break;

    case kHotkeyIDHistory:
        if (g_history) {
            QTimer::singleShot(0, []() {
                g_history->refreshHistory();
                g_history->show();
                g_history->raise();
                g_history->activateWindow();
            });
        }
        break;

    default:
        break;
    }

    return noErr;
}

void registerGlobalHotkey() {
    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind  = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    EventHotKeyRef hotKeyRef;
    EventHotKeyID  hotKeyID;
    hotKeyID.signature = 'SNPF';

    // Cmd+Shift+S — Screenshot (kVK_ANSI_S = 0x01)
    hotKeyID.id = kHotkeyIDScreenshot;
    RegisterEventHotKey(0x01, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &hotKeyRef);

    // Cmd+Shift+R — Record (kVK_ANSI_R = 0x0F)
    hotKeyID.id = kHotkeyIDRecord;
    RegisterEventHotKey(0x0F, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &hotKeyRef);

    // Cmd+Shift+H — History (kVK_ANSI_H = 0x04)
    hotKeyID.id = kHotkeyIDHistory;
    RegisterEventHotKey(0x04, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &hotKeyRef);
}
#endif

static void saveImage(const QImage &img) {
    if (img.isNull()) return;
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);

    char *saveDir = snapforge_default_save_path();
    QString dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Pictures/Snapforge";
    if (saveDir) snapforge_free_string(saveDir);

    QDir().mkpath(dir);
    QString timestamp = QDateTime::currentDateTime().toString("yyyy-MM-dd_HH-mm-ss");
    QString path = dir + "/screenshot_" + timestamp + ".png";

    int result = snapforge_save_image(rgba.constBits(), rgba.width(), rgba.height(),
                                      path.toUtf8().constData(), 0, 90);
    if (result == 0) {
        qDebug("Saved: %s", qPrintable(path));
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

    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
    app.setQuitOnLastWindowClosed(false);

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

    // Handle screenshot save
    QObject::connect(&overlay, &OverlayWindow::screenshotReady,
                     [](QImage composited, int /*w*/, int /*h*/) {
        saveImage(composited);
    });

    // Handle clipboard copy
    QObject::connect(&overlay, &OverlayWindow::clipboardReady,
                     [](QImage composited, int /*w*/, int /*h*/) {
        copyImage(composited);
    });

    // Create manager/window instances
    RecordingManager recording;

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
    g_prefs = &prefs;

    // System tray
    QSystemTrayIcon tray;
    tray.setIcon(QIcon::fromTheme("camera-photo"));
    tray.setToolTip("Snapforge");

    QMenu *menu = new QMenu();

    auto buildNormalMenu = [&]() {
        menu->clear();

        menu->addAction("Screenshot (Cmd+Shift+S)", &overlay, &OverlayWindow::activate);

        menu->addAction("Record (Cmd+Shift+R)", [&]() {
            if (recording.isRecording()) {
                recording.stopRecording();
            } else {
                overlay.activateForRecording();
            }
        });

        menu->addSeparator();

        menu->addAction("History (Cmd+Shift+H)", [&]() {
            history.refreshHistory();
            history.show();
            history.raise();
            history.activateWindow();
        });

        menu->addAction("Preferences", [&]() {
            prefs.show();
            prefs.raise();
            prefs.activateWindow();
        });

        menu->addSeparator();

        menu->addAction("Quit", &app, &QApplication::quit);
    };

    auto buildRecordingMenu = [&]() {
        menu->clear();

        QAction *recordingLabel = menu->addAction("● Recording...");
        recordingLabel->setEnabled(false);

        menu->addAction("■ Stop Recording", [&]() {
            recording.stopRecording();
        });

        menu->addSeparator();

        menu->addAction("History (Cmd+Shift+H)", [&]() {
            history.refreshHistory();
            history.show();
            history.raise();
            history.activateWindow();
        });

        menu->addAction("Preferences", [&]() {
            prefs.show();
            prefs.raise();
            prefs.activateWindow();
        });

        menu->addSeparator();

        menu->addAction("Quit", &app, &QApplication::quit);
    };

    buildNormalMenu();
    tray.setContextMenu(menu);
    tray.show();

    // Update tray menu when recording starts/stops
    QObject::connect(&recording, &RecordingManager::recordingStarted,
                     [&](const QString &/*path*/) {
        buildRecordingMenu();
    });

    QObject::connect(&recording, &RecordingManager::recordingStopped,
                     [&](const QString &/*path*/) {
        buildNormalMenu();
    });

#ifdef Q_OS_MAC
    registerGlobalHotkey();
#endif

    return app.exec();
}
