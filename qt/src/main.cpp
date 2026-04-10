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

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
    app.setQuitOnLastWindowClosed(false);

#ifdef Q_OS_MAC
    // Hide from dock (accessory app)
    app.setAttribute(Qt::AA_DontShowIconsInMenus);
#endif

    // Request permission if needed
    if (!snapforge_has_permission()) {
        snapforge_request_permission();
    }

    // Pre-create overlay and warm up the window server registration
    OverlayWindow overlay;
    g_overlay = &overlay;
    overlay.showFullScreen();
    overlay.hide();

    // Handle region capture — save to file
    QObject::connect(&overlay, &OverlayWindow::regionCaptured,
                     [](int x, int y, int w, int h) {
        CapturedImage img = snapforge_capture_region(0, x, y, w, h);
        if (!img.data || img.width == 0) return;

        // Generate save path
        char *saveDir = snapforge_default_save_path();
        QString dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Pictures/Snapforge";
        if (saveDir) snapforge_free_string(saveDir);

        QDir().mkpath(dir);
        QString timestamp = QDateTime::currentDateTime().toString("yyyy-MM-dd_HH-mm-ss");
        QString path = dir + "/screenshot_" + timestamp + ".png";

        int result = snapforge_save_image(img.data, img.width, img.height,
                                          path.toUtf8().constData(), 0, 90);
        snapforge_free_buffer(img.data, img.len);

        if (result == 0) {
            qDebug("Saved: %s", qPrintable(path));
        }
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
