#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include <QIcon>
#include <QPixmap>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QClipboard>
#include <QMimeData>
#include <QUrl>
#include <QEvent>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include "OverlayWindow.h"
#include "RecordingManager.h"
#include "HistoryWindow.h"
#include "PreferencesWindow.h"
#include "snapforge_ffi.h"
#ifdef Q_OS_MAC
#include "WorkspaceSleepObserver.h"
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
static PreferencesWindow *g_prefs     = nullptr;

// Hotkey IDs
static const UInt32 kHotkeyIDScreenshot = 1;
static const UInt32 kHotkeyIDRecord     = 2;
static const UInt32 kHotkeyIDHistory    = 3;

// H6: track every registered hotkey so we can unregister them on quit.
// Previously we only kept the last one, so two were leaked on app exit.
static std::array<EventHotKeyRef, 3> g_hotkeys = { nullptr, nullptr, nullptr };

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef event, void *) {
    EventHotKeyID firedID;
    GetEventParameter(event, kEventParamDirectObject,
                      typeEventHotKeyID, nullptr,
                      sizeof(firedID), nullptr, &firedID);

    // Fix #14: ignore any hotkey that would re-enter the overlay while it's
    // already on screen. Repeat presses would otherwise stack activations
    // and double-capture the screen.
    const bool overlayBusy = g_overlay && g_overlay->isVisible();

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
            else QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
        }
        break;

    case kHotkeyIDRecord:
        if (g_recording && g_overlay) {
            if (g_recording->isRecording()) {
                QTimer::singleShot(0, g_recording, &RecordingManager::stopRecording);
            } else if (overlayBusy) {
                notifyBusy();
            } else {
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

void registerGlobalHotkey() {
    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind  = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    EventHotKeyID  hotKeyID;
    hotKeyID.signature = 'SNPF';

    // Cmd+Shift+S — Screenshot (kVK_ANSI_S = 0x01)
    hotKeyID.id = kHotkeyIDScreenshot;
    RegisterEventHotKey(0x01, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &g_hotkeys[0]);

    // Cmd+Shift+R — Record (kVK_ANSI_R = 0x0F)
    hotKeyID.id = kHotkeyIDRecord;
    RegisterEventHotKey(0x0F, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &g_hotkeys[1]);

    // Cmd+Shift+H — History (kVK_ANSI_H = 0x04)
    hotKeyID.id = kHotkeyIDHistory;
    RegisterEventHotKey(0x04, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &g_hotkeys[2]);
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

    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
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
    g_prefs = &prefs;
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

    // Idle tray icon — brand mark in white on a transparent background:
    // two opposing corner brackets (top-left + bottom-right) with a diagonal
    // slash, matching the original Snapforge logo glyph.
    auto makeIdleIcon = []() -> QIcon {
        const qreal logicalSz = 22.0;
        const qreal dpr = 2.0;
        const int sz = static_cast<int>(logicalSz * dpr);
        QPixmap pm(sz, sz);
        pm.setDevicePixelRatio(dpr);
        pm.fill(Qt::transparent);

        QPainter p(&pm);
        p.setRenderHint(QPainter::Antialiasing);

        const qreal margin = 2.5;
        const qreal hook = 6.5;     // length of each bracket arm
        const qreal stroke = 2.0;
        const qreal x0 = margin;
        const qreal y0 = margin;
        const qreal x1 = logicalSz - margin;
        const qreal y1 = logicalSz - margin;

        QPen pen(QColor(255, 255, 255, 255), stroke);
        pen.setCapStyle(Qt::RoundCap);
        pen.setJoinStyle(Qt::RoundJoin);
        p.setPen(pen);
        p.setBrush(Qt::NoBrush);

        // Top-left bracket ┌
        QPainterPath tl;
        tl.moveTo(x0, y0 + hook);
        tl.lineTo(x0, y0);
        tl.lineTo(x0 + hook, y0);
        p.drawPath(tl);

        // Bottom-right bracket ┘
        QPainterPath br;
        br.moveTo(x1 - hook, y1);
        br.lineTo(x1, y1);
        br.lineTo(x1, y1 - hook);
        p.drawPath(br);

        // Diagonal slash (top-right → bottom-left direction per original logo)
        const qreal slashInset = 4.5;
        p.drawLine(QPointF(x1 - slashInset, y0 + slashInset),
                   QPointF(x0 + slashInset, y1 - slashInset));

        p.end();
        // Non-template so the icon stays white regardless of macOS menu-bar mode.
        return QIcon(pm);
    };

    // Single wide recording pill icon — renders the full mockup `.rec-indicator`
    // (red dot + "REC" label + "mm:ss" timer + pause glyph + stop glyph) inside
    // one rounded container, displayed as a single NSStatusItem in the menu bar.
    // Interactive actions (Pause/Resume/Stop) are exposed through the tray's
    // context menu when clicked.
    auto makeRecordingPillIcon = [](double alpha, bool paused, int seconds) -> QIcon {
        // Match mockup spacing: dot — REC — time — | pause | stop
        const int logicalW = 156;
        const int logicalH = 22;
        const qreal dpr = 2.0;
        QPixmap pm(static_cast<int>(logicalW * dpr), static_cast<int>(logicalH * dpr));
        pm.setDevicePixelRatio(dpr);
        pm.fill(Qt::transparent);
        QPainter p(&pm);
        p.setRenderHint(QPainter::Antialiasing);

        // Pill background — mockup: rgba(12,14,18,0.92), red border 0.3 alpha,
        // border-radius: 24px (fully rounded at this height).
        const qreal radius = logicalH / 2.0;
        QRectF pillRect(0.5, 0.5, logicalW - 1.0, logicalH - 1.0);
        QPainterPath pill;
        pill.addRoundedRect(pillRect, radius, radius);
        p.fillPath(pill, QColor(12, 14, 18, 235));
        p.setPen(QPen(QColor(239, 68, 68, 90), 1.0));
        p.setBrush(Qt::NoBrush);
        p.drawPath(pill);

        qreal x = 10.0;
        const qreal cy = logicalH / 2.0;

        // Red dot / pause glyph (left of label)
        if (paused) {
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(239, 68, 68, 220));
            p.drawRoundedRect(QRectF(x, cy - 4.5, 2.5, 9.0), 1.0, 1.0);
            p.drawRoundedRect(QRectF(x + 4.5, cy - 4.5, 2.5, 9.0), 1.0, 1.0);
            x += 11.0;
        } else {
            // Glow halo
            int halo = qBound(0, static_cast<int>(70 * alpha), 70);
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(239, 68, 68, halo));
            p.drawEllipse(QPointF(x + 5.0, cy), 7.5, 7.5);
            // Solid red dot
            int dotAlpha = qBound(120, static_cast<int>(255 * alpha), 255);
            p.setBrush(QColor(239, 68, 68, dotAlpha));
            p.drawEllipse(QPointF(x + 5.0, cy), 5.0, 5.0);
            x += 11.0;
        }

        // REC label — mockup .rec-label: 10px mono, uppercase, red, letter-spacing: 1
        QFont labelFont(QStringLiteral("Menlo"));
        labelFont.setPixelSize(10);
        labelFont.setWeight(QFont::Bold);
        labelFont.setLetterSpacing(QFont::AbsoluteSpacing, 1.0);
        p.setFont(labelFont);
        p.setPen(paused ? QColor(239, 68, 68, 180) : QColor(239, 68, 68, 255));
        QString recLabel = paused ? QStringLiteral("PAUSED") : QStringLiteral("REC");
        QFontMetrics lfm(labelFont);
        qreal labelW = lfm.horizontalAdvance(recLabel);
        p.drawText(QRectF(x + 4, 0, labelW + 6, logicalH),
                   Qt::AlignVCenter | Qt::AlignLeft, recLabel);
        x += labelW + 10.0;

        // Timer — mockup .rec-time: 16px mono, white, tabular numerals
        QFont timeFont(QStringLiteral("Menlo"));
        timeFont.setPixelSize(14);
        timeFont.setWeight(QFont::DemiBold);
        p.setFont(timeFont);
        p.setPen(paused ? QColor(200, 200, 200, 230) : QColor(245, 245, 245, 255));
        QString timeStr = QStringLiteral("%1:%2")
            .arg(seconds / 60, 2, 10, QLatin1Char('0'))
            .arg(seconds % 60, 2, 10, QLatin1Char('0'));
        QFontMetrics tfm(timeFont);
        qreal timeW = tfm.horizontalAdvance(timeStr);
        p.drawText(QRectF(x, 0, timeW + 4, logicalH),
                   Qt::AlignVCenter | Qt::AlignLeft, timeStr);
        x += timeW + 8.0;

        // Subtle separator before the control glyphs
        p.setPen(QPen(QColor(239, 68, 68, 40), 1.0));
        p.drawLine(QPointF(x, 5.0), QPointF(x, logicalH - 5.0));
        x += 8.0;

        // Pause / Play glyph — mockup .rec-stop colour (#ff8888)
        const QColor ctrlColor(255, 136, 136, 230);
        p.setPen(Qt::NoPen);
        p.setBrush(ctrlColor);
        qreal pauseCx = x + 5.0;
        if (paused) {
            QPainterPath tri;
            tri.moveTo(pauseCx - 3.0, cy - 5.0);
            tri.lineTo(pauseCx + 4.5, cy);
            tri.lineTo(pauseCx - 3.0, cy + 5.0);
            tri.closeSubpath();
            p.drawPath(tri);
        } else {
            p.drawRoundedRect(QRectF(pauseCx - 4.0, cy - 5.0, 2.8, 10.0), 1.0, 1.0);
            p.drawRoundedRect(QRectF(pauseCx + 1.2, cy - 5.0, 2.8, 10.0), 1.0, 1.0);
        }
        x += 14.0;

        // Stop glyph — filled square (.rec-stop-icon)
        p.setBrush(ctrlColor);
        p.drawRoundedRect(QRectF(x - 1.0, cy - 4.5, 9.0, 9.0), 1.2, 1.2);

        p.end();
        return QIcon(pm);
    };

    qDebug("System tray available: %d", QSystemTrayIcon::isSystemTrayAvailable());
    QSystemTrayIcon tray;
    const QIcon idleIcon = makeIdleIcon();
    tray.setIcon(idleIcon);
    tray.setToolTip("Snapforge");
    app.setProperty("systemTray", QVariant::fromValue(static_cast<QObject *>(&tray)));

    // Q2: surface invalid-selection feedback (too small, multi-display) via
    // the tray instead of silently dropping. Wired here because `tray` needs
    // to exist first.
    QObject::connect(&overlay, &OverlayWindow::regionInvalid,
                     [&tray](const QString &reason) {
        tray.showMessage("Snapforge — Selection invalid",
                         reason,
                         QSystemTrayIcon::Information, 3000);
    });

    // Stack-allocated so the menu is destroyed deterministically when main returns.
    // QMenu requires a QWidget parent and QSystemTrayIcon is a QObject, so we can't
    // hand off parent-ownership to the tray; manage the lifetime here instead.
    QMenu menuObj;
    QMenu *menu = &menuObj;

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

        QAction *recordingLabel = menu->addAction(
            recording.isPaused() ? "⏸ Paused" : "● Recording...");
        recordingLabel->setEnabled(false);

        if (recording.isPaused()) {
            menu->addAction("▶ Resume Recording", [&]() {
                recording.resumeRecording();
            });
        } else {
            menu->addAction("⏸ Pause Recording", [&]() {
                recording.pauseRecording();
            });
        }

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
    qDebug("Tray shown; visible=%d", tray.isVisible());

    // Pulse the recording pill icon twice per second (matches mockup recPulse).
    // Also re-renders on every tick so the elapsed timer stays current.
    auto *pulseTimer = new QTimer(&app);
    pulseTimer->setInterval(500);
    bool pulseHigh = true;
    auto refreshPill = [&, makeRecordingPillIcon](double alpha) {
        tray.setIcon(makeRecordingPillIcon(alpha, recording.isPaused(),
                                           recording.elapsedSeconds()));
    };
    QObject::connect(pulseTimer, &QTimer::timeout, [&, refreshPill]() mutable {
        pulseHigh = !pulseHigh;
        refreshPill(pulseHigh ? 1.0 : 0.35);
    });

    // Update the timer text every second when elapsedChanged fires.
    QObject::connect(&recording, &RecordingManager::elapsedChanged,
                     [refreshPill, &recording](int /*secs*/) {
        if (recording.isRecording() || recording.isPaused()) {
            refreshPill(1.0);
        }
    });

    // Pause: stop the pulse animation, re-render the pill with "PAUSED" state.
    QObject::connect(&recording, &RecordingManager::recordingPaused,
                     [&, refreshPill]() {
        pulseTimer->stop();
        refreshPill(1.0);
        tray.setToolTip("Snapforge — Paused");
        buildRecordingMenu();
    });
    QObject::connect(&recording, &RecordingManager::recordingResumed,
                     [&, refreshPill]() {
        pulseTimer->start();
        refreshPill(1.0);
        tray.setToolTip("Snapforge — Recording");
        buildRecordingMenu();
    });

    // On start: swap the tray icon to the recording pill and kick off the pulse.
    QObject::connect(&recording, &RecordingManager::recordingStarted,
                     [&, refreshPill](const QString &/*path*/) {
        buildRecordingMenu();
        tray.setToolTip("Snapforge — Recording");
        refreshPill(1.0);
        pulseTimer->start();
    });

    QObject::connect(&recording, &RecordingManager::recordingStopped,
                     [&](const QString &path) {
        pulseTimer->stop();
        tray.setIcon(idleIcon);
        tray.setToolTip("Snapforge");
        buildNormalMenu();

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
    QObject::connect(&recording, &RecordingManager::recordingError,
                     [&](const QString &message) {
        qWarning("Recording error: %s", qPrintable(message));
        pulseTimer->stop();
        tray.setIcon(idleIcon);
        tray.setToolTip("Snapforge");
        tray.showMessage("Snapforge — Recording Failed", message,
                         QSystemTrayIcon::Warning, 5000);
    });

#ifdef Q_OS_MAC
    registerGlobalHotkey();
    QObject::connect(&app, &QCoreApplication::aboutToQuit, []() {
        unregisterGlobalHotkeys();
    });
#endif

    int rc = app.exec();

#ifdef Q_OS_MAC
    // Clear globals so any late-arriving Carbon hotkey event (unlikely after
    // app.exec returns, but possible during teardown) sees null pointers
    // instead of dangling stack objects.
    g_overlay = nullptr;
    g_recording = nullptr;
    g_history = nullptr;
    g_prefs = nullptr;
#endif
    g_prefsRef = nullptr;

    return rc;
}
