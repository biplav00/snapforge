#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include "OverlayWindow.h"
#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>
#include <objc/runtime.h>
#include <objc/message.h>

// macOS global hotkey callback
static OverlayWindow *g_overlay = nullptr;

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef, void *) {
    if (g_overlay) {
        // Use QTimer to ensure we're on the Qt event loop
        QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
    }
    return noErr;
}

void registerGlobalHotkey() {
    EventHotKeyRef hotKeyRef;
    EventHotKeyID hotKeyID;
    hotKeyID.signature = 'SNPF';
    hotKeyID.id = 1;

    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    // Cmd+Shift+S = kVK_ANSI_S (0x01), modifiers: cmdKey | shiftKey
    RegisterEventHotKey(0x01, cmdKey | shiftKey, hotKeyID,
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

    // System tray
    QSystemTrayIcon tray;
    tray.setIcon(QIcon::fromTheme("camera-photo"));
    tray.setToolTip("Snapforge");

    QMenu menu;
    menu.addAction("Screenshot (Cmd+Shift+S)", &overlay, &OverlayWindow::activate);
    menu.addSeparator();
    menu.addAction("Quit", &app, &QApplication::quit);
    tray.setContextMenu(&menu);
    tray.show();

#ifdef Q_OS_MAC
    registerGlobalHotkey();
#endif

    return app.exec();
}
