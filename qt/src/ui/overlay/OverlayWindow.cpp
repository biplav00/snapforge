#include "OverlayWindow.h"
#include "AnnotationCanvas.h"
#include "AnnotationToolbar.h"
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QScreen>
#include <QWindow>
#include <QGuiApplication>
#include <QCursor>
#include <QElapsedTimer>
#include <QTimer>
#include <QFontMetrics>
#include <QPushButton>
#include <QPointer>
#include <QtConcurrent/QtConcurrent>
#include <QFutureWatcher>
#include <QJsonDocument>
#include <QJsonObject>
#include <algorithm>
#include <cmath>
#include "SnapforgeClient.h"
#include "Shortcuts.h"
#include "HotkeyKeyMap.h"
#include <QApplication>

#ifdef Q_OS_MACOS
#include <objc/runtime.h>
#include <objc/message.h>

// Set the native crosshair NSCursor directly. On the borderless overlay panel
// AppKit resets the cursor to the default arrow on every pointer move (no
// tracking area/cursor rect is registered), and neither Qt's override cursor
// nor a per-widget setCursor() survives that re-evaluation. A native
// [NSCursor set] from inside the move handler sticks until the next reset, and
// since motion is a continuous stream of moves the crosshair holds.
static void setNativeCrosshair() {
    id cur = ((id (*)(id, SEL))objc_msgSend)(
        (id)objc_getClass("NSCursor"), sel_registerName("crosshairCursor"));
    ((void (*)(id, SEL))objc_msgSend)(cur, sel_registerName("set"));
}
#else
static void setNativeCrosshair() {}
#endif

// --- Rebindable overlay shortcuts ---
//
// Every row the Preferences Hotkeys tab offers under tools/sizes/actions is
// resolved here against the shared config (hotkeys.<section>.<actionId>),
// falling back to the historical hardcoded key. Chords are parsed once per
// reloadKeyBindings(); keyPressEvent then matches QKeyEvents against them.

bool OverlayWindow::KeyBinding::matches(const QKeyEvent *event) const {
    if (key == 0)
        return false;
    const Qt::KeyboardModifiers evMods = event->modifiers() &
        (Qt::ControlModifier | Qt::ShiftModifier | Qt::AltModifier | Qt::MetaModifier);
    if (evMods != mods)
        return false;
    if (event->key() == key)
        return true;
    // Prefs records keypad-Enter and Return both as "Enter" — accept either.
    if (key == Qt::Key_Return && event->key() == Qt::Key_Enter)
        return true;
    // Prefs records Shift+digit/punctuation as the unshifted key
    // ("Shift+1"), but macOS delivers the shifted character (Key_Exclam).
    if (evMods & Qt::ShiftModifier)
        return hotkeykeymap::unshiftedKey(event->key(), evMods,
                                          event->nativeVirtualKey()) == key;
    return false;
}

OverlayWindow::KeyBinding OverlayWindow::bindingFor(const shortcuts::Snapshot &cfg,
                                                    const char *section,
                                                    const char *actionId,
                                                    const char *defaultChord) {
    KeyBinding b;
    const QString c = cfg.chord(QLatin1String(section),
                                QLatin1String(actionId),
                                QLatin1String(defaultChord));
    if (!shortcuts::toQtKey(c, &b.key, &b.mods)) {
        // Unparseable user chord — fall back to the built-in default so the
        // action never goes dead.
        shortcuts::toQtKey(QLatin1String(defaultChord), &b.key, &b.mods);
    }
    return b;
}

void OverlayWindow::reloadKeyBindings() {
    // Defaults mirror the PreferencesWindow hotkey table (and the keys that
    // used to be hardcoded here).
    static const struct { const char *id; const char *def; ToolType tool; } kTools[] = {
        {"arrow",       "A", ToolType::Arrow},
        {"rect",        "R", ToolType::Rect},
        {"circle",      "C", ToolType::Circle},
        {"line",        "L", ToolType::Line},
        {"dottedline",  "D", ToolType::DottedLine},
        {"freehand",    "F", ToolType::Freehand},
        {"text",        "T", ToolType::Text},
        {"highlight",   "H", ToolType::Highlight},
        {"blur",        "B", ToolType::Blur},
        {"steps",       "N", ToolType::Steps},
        {"colorpicker", "I", ToolType::ColorPicker},
        {"measure",     "M", ToolType::Measure},
    };
    // One Snapshot for the whole batch — ~20 lookups would otherwise each
    // re-load and re-parse the config from disk.
    const shortcuts::Snapshot cfg;

    m_toolBindings.clear();
    for (const auto &t : kTools)
        m_toolBindings.append(qMakePair(bindingFor(cfg, "tools", t.id, t.def), t.tool));

    m_bindSizeSmall  = bindingFor(cfg, "sizes", "small",  "1");
    m_bindSizeMedium = bindingFor(cfg, "sizes", "medium", "2");
    m_bindSizeLarge  = bindingFor(cfg, "sizes", "large",  "3");

    m_bindSave   = bindingFor(cfg, "actions", "save",   "Cmd+S");
    m_bindCopy   = bindingFor(cfg, "actions", "copy",   "Cmd+C");
    m_bindUndo   = bindingFor(cfg, "actions", "undo",   "Cmd+Z");
    m_bindRedo   = bindingFor(cfg, "actions", "redo",   "Cmd+Shift+Z");
    m_bindCancel   = bindingFor(cfg, "actions", "cancel",     "Escape");
    m_bindForceQuit = bindingFor(cfg, "actions", "forcequit",  "Cmd+Shift+C");
}

// --- OverlayWindow ---

OverlayWindow::OverlayWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool)
{
    setCursor(Qt::CrossCursor);
    setMouseTracking(true);

    // Initial read of the rebindable overlay shortcuts; main.cpp re-invokes
    // reloadKeyBindings() whenever Preferences saves.
    reloadKeyBindings();

    // Pre-warm font metrics to avoid 125ms alias scan on first paint
    QFont font("Menlo", 11);
    QFontMetrics fm(font);
    fm.boundingRect("0");

    // This window is created once and reused for the whole session (see
    // main.cpp). React to display-topology changes so a sleep/wake or monitor
    // hot-plug can't strand the reused window on a now-stale screen. Each
    // activation re-anchors anyway; these keep a *visible* overlay correct if
    // the topology shifts while it's up.
    connect(qApp, &QGuiApplication::screenAdded,
            this, &OverlayWindow::onScreenConfigChanged);
    connect(qApp, &QGuiApplication::screenRemoved,
            this, &OverlayWindow::onScreenConfigChanged);
    connect(qApp, &QGuiApplication::primaryScreenChanged,
            this, &OverlayWindow::onScreenConfigChanged);
}

void OverlayWindow::repositionToScreen(QScreen *screen) {
    if (!screen)
        return;
    const QRect geo = screen->geometry();

    // Update the QWidget's logical geometry first (child layout, hit-testing,
    // and selectedRect() math all key off this).
    setGeometry(geo);

    // The catch: this overlay is a single long-lived window dismissed with
    // hide(), never destroyed, so its native NSWindow persists for the whole
    // session. Across a display reconfiguration (sleep/wake, monitor hot-plug,
    // resolution/arrangement change) AppKit can silently relocate that hidden
    // window to another display, while Qt's cached geometry still believes it
    // is where we last put it. The setGeometry() above then no-ops (cached
    // rect already equals geo), leaving the window on the wrong monitor until
    // the app restarts — the reported bug.
    //
    // Re-anchor at the platform layer: bind the native window to the target
    // QScreen and push the geometry straight through QWindow, which bypasses
    // QWidget's no-op cache. windowHandle() is null only before the first
    // show(); the first activation always lands correctly on a fresh launch,
    // and every reuse afterwards has a live handle (hide() keeps it).
    if (QWindow *wh = windowHandle()) {
        if (wh->screen() != screen)
            wh->setScreen(screen);
        wh->setGeometry(geo);
    }
}

void OverlayWindow::onScreenConfigChanged() {
    // Topology changed underneath us. If the overlay is idle and hidden, do
    // nothing: the next activate() repositions to the cursor's screen from
    // scratch. If a session is live (visible, or its capture still pending),
    // cancel it: the captured frame, the display index it came from and the
    // window-local selection all describe a layout that no longer exists, so
    // moving the window would composite/crop the wrong pixels.
    if (!isVisible() && !m_captureInFlight)
        return;
    qWarning("Overlay: display configuration changed mid-session — cancelling");
    cancelSession();
}

void OverlayWindow::cancelSession() {
    setCursor(Qt::ArrowCursor);
    if (m_canvas) m_canvas->cancelTextInput();
    m_annotationState.clearAnnotations();
    m_resizing = false;
    m_activeResize = EdgeNone;
    exitAnnotateMode();
    exitRecordSelectMode();
    m_mode = Select;
    m_hasRegion = false;
    m_drawing = false;
    hideOverlay();
    emit cancelled();
}

// Shared internal activation logic
void OverlayWindow::activateInternal() {

    // A previous session still on screen would end up in the new capture
    // frame (the overlay is opaque) — take it down first.
    if (isVisible()) {
        hide();
        syncSelectCursor();
    }

    m_drawing = false;
    m_hasRegion = false;
    m_resizing = false;
    m_activeResize = EdgeNone;
    m_annotationState.clearAnnotations();
    exitAnnotateMode();
    exitRecordSelectMode();

    // Restore last region if "remember region" is enabled
    if (m_rememberRegion && (m_lastStartPos != m_lastEndPos)) {
        m_startPos = m_lastStartPos;
        m_endPos = m_lastEndPos;
    } else {
        m_startPos = QPoint();
        m_endPos = QPoint();
    }

    // Detect which display the user is on (cursor position), NOT primary —
    // otherwise capturing a windowed app on a secondary display grabs the
    // primary display's current Space instead.
    QPoint cursor = QCursor::pos();
    QScreen *screen = QGuiApplication::screenAt(cursor);
    if (!screen) screen = QGuiApplication::primaryScreen();
    if (!screen) {
        m_displayIndex = 0;
    } else {
        int display = sf::displayAtPoint(cursor.x(), cursor.y());
        m_displayIndex = (display >= 0) ? display : 0;
    }

    // Always clear previous capture so a failed fresh capture can't
    // silently reuse the last activation's image (which would show the
    // wrong window when switching apps between screenshots).
    m_screenshot = QImage();
    m_captureInFlight = true;
    const quint64 captureSeq = ++m_captureSeq;

    // Size overlay to the cursor's screen (not primary). repositionToScreen
    // re-anchors the reused native window at the platform layer so a stale
    // post-reconfiguration screen association can't strand it on the wrong
    // monitor (see the helper for the full rationale).
    if (screen) {
        repositionToScreen(screen);
    }

    // A remembered region was captured in a previous display's coordinate space.
    // If that display was larger, the restored points can now fall outside this
    // overlay — clamp them (now that repositionToScreen has sized us to the
    // current display) so the selection, and the pixel region we later hand the
    // encoder or the screenshot cropper, stays within bounds. No-op for the
    // fresh-activation default (both points 0,0).
    if (m_startPos != m_endPos) {
        const int maxX = std::max(0, width() - 1);
        const int maxY = std::max(0, height() - 1);
        m_startPos.setX(std::clamp(m_startPos.x(), 0, maxX));
        m_startPos.setY(std::clamp(m_startPos.y(), 0, maxY));
        m_endPos.setX(std::clamp(m_endPos.x(), 0, maxX));
        m_endPos.setY(std::clamp(m_endPos.y(), 0, maxY));
    }

    // Capture FIRST, show the overlay only once the frame has landed. The
    // overlay is an opaque window (no WA_TranslucentBackground): showing it
    // before the grab let the capture race the window server and record the
    // overlay itself (or a flat dim) instead of the desktop. The capture runs
    // on a worker so the UI thread never blocks on SCK; the hotkey gate stays
    // closed meanwhile because isBusy() reports a pending capture.
    m_activationTimer.start();
    int displayIndex = m_displayIndex;
    QPointer<OverlayWindow> self(this);
    auto *watcher = new QFutureWatcher<QImage>(this);
    connect(watcher, &QFutureWatcher<QImage>::finished, this, [self, watcher, displayIndex, captureSeq]() {
        QImage img = watcher->result();
        watcher->deleteLater();
        if (!self) return;
        // Superseded by a newer activation: that activation owns
        // m_captureInFlight now, so leave all state untouched.
        if (self->m_captureSeq != captureSeq) return;
        // This is the in-flight capture resolving — clear the gate regardless
        // of outcome so a failure can't leave isBusy() stuck true.
        self->m_captureInFlight = false;
        // A dismissed session bumps m_captureSeq (hideOverlay), so reaching
        // here means this activation is still current. Defensive: the display
        // must still be the one we captured.
        if (self->m_displayIndex != displayIndex) return;
        if (img.isNull()) {
            // Capture failed (SCK timeout, CGDisplayCreateImage null/all-zero,
            // or revoked Screen Recording permission). Recover the overlay
            // instead of leaving a dim crosshair that never resolves and wedges
            // the hotkey gate until the app is restarted.
            self->handleCaptureFailure("snapforge_capture_fullscreen returned null");
            return;
        }
        self->m_screenshot = img;
        // If annotate mode was entered BEFORE the capture resolved (remembered
        // region via activate(), or activateFullscreen()), the canvas was built
        // with a null screenshot and its crop is empty — composite would be
        // blank. Now that the real capture landed, re-feed it and re-crop the
        // region so the committed image isn't blank.
        if (self->m_mode == Annotate && self->m_canvas) {
            self->m_canvas->setScreenshot(img);
            QRect sel = self->selectedRect();
            self->m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
        }
        self->presentOverlay();
    });
    QFuture<QImage> future = QtConcurrent::run([displayIndex]() -> QImage {
        // The client copies the pixels into the QImage and frees the FFI
        // buffer internally; nullopt (capture failed) -> null QImage.
        return sf::captureFullscreen(static_cast<uint32_t>(displayIndex))
            .value_or(QImage());
    });
    watcher->setFuture(future);

    // Watchdog. The worker carries its own SCK/CGDisplay timeouts (~5s), so the
    // finished handler normally fires within ~6s even on failure. But if the
    // worker thread itself wedges (capture daemon deadlock) the handler may
    // never run, leaving m_captureInFlight stuck true and the hotkey gate dead.
    // Force recovery past that window. Guarded by captureSeq so a later
    // activation isn't torn down, and by m_captureInFlight so a capture that
    // already resolved is a no-op.
    QTimer::singleShot(8000, this, [self, captureSeq]() {
        if (!self) return;
        if (self->m_captureSeq != captureSeq) return; // superseded by newer activation
        if (!self->m_captureInFlight) return;         // already resolved (success or fail)
        self->handleCaptureFailure("capture watchdog timeout (worker stalled)");
    });

}

// Put the (already captured) session on screen. Called from the capture
// completion handler only — see activateInternal for why the overlay must not
// be visible while the frame is being grabbed.
void OverlayWindow::presentOverlay() {
    show();
    activateWindow();
    raise();

#ifdef Q_OS_MACOS
    {
        auto *nsView = reinterpret_cast<id>(winId());
        id nsWindow = ((id (*)(id, SEL))objc_msgSend)(nsView, sel_registerName("window"));
        if (nsWindow) {
            ((void (*)(id, SEL, long))objc_msgSend)(nsWindow, sel_registerName("setLevel:"), kOverlayWindowLevel);
            unsigned long behavior = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
            ((void (*)(id, SEL, unsigned long))objc_msgSend)(
                nsWindow, sel_registerName("setCollectionBehavior:"), behavior);

            // Lock the NSWindow against user-initiated edge-drag move/resize.
            // Qt::Tool gives macOS a resizable utility-window border by default.
            ((void (*)(id, SEL, BOOL))objc_msgSend)(
                nsWindow, sel_registerName("setMovable:"), NO);
            ((void (*)(id, SEL, BOOL))objc_msgSend)(
                nsWindow, sel_registerName("setMovableByWindowBackground:"), NO);
            // Qt::Tool maps to NSPanel; NSPanel.hidesOnDeactivate defaults YES,
            // which makes AppKit auto-hide the overlay the moment Snapforge
            // loses focus (e.g. user clicks an input field in another app, or
            // switches Space). The hide bypasses Qt, leaving isVisible() in a
            // stale state and the hotkey gate stuck on "busy". Pin it visible.
            ((void (*)(id, SEL, BOOL))objc_msgSend)(
                nsWindow, sel_registerName("setHidesOnDeactivate:"), NO);
            // Clear all NSWindowStyleMask bits that imply resizability.
            ((void (*)(id, SEL, unsigned long))objc_msgSend)(
                nsWindow, sel_registerName("setStyleMask:"), 0UL); // NSWindowStyleMaskBorderless

            // Activate app + make key window (required for accessory apps after hide)
            id nsApp = ((id (*)(id, SEL))objc_msgSend)(
                (id)objc_getClass("NSApplication"), sel_registerName("sharedApplication"));
            if (nsApp) {
                if (((BOOL (*)(id, SEL, SEL))objc_msgSend)(nsApp, sel_registerName("respondsToSelector:"),
                        sel_registerName("activate"))) {
                    ((void (*)(id, SEL))objc_msgSend)(nsApp, sel_registerName("activate"));
                } else {
                    ((void (*)(id, SEL, BOOL))objc_msgSend)(nsApp, sel_registerName("activateIgnoringOtherApps:"), YES);
                }
            }
            ((void (*)(id, SEL))objc_msgSend)(nsWindow, sel_registerName("makeKeyWindow"));
        }
    }
#endif

    // Annotate mode may have been entered while hidden (remembered region /
    // fullscreen): make sure its widgets sit on top now that we're visible.
    if (m_mode == Annotate) {
        if (m_canvas) m_canvas->raise();
        if (m_toolbar) m_toolbar->raise();
    }

    // Apply the draw-region crosshair now that the window is shown and (on macOS)
    // key. Uses an application override cursor so it survives the AppKit cursor
    // re-evaluations that a stationary pointer would otherwise lose it to.
    syncSelectCursor();
    update();

    qDebug("Overlay shown in %lld ms", m_activationTimer.isValid()
                                           ? m_activationTimer.elapsed() : -1LL);
}

void OverlayWindow::activate() {
    m_purpose = Screenshot;
    activateInternal();
    // If we restored a remembered region, go straight to annotate mode
    if (m_rememberRegion && (m_startPos != m_endPos)) {
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            m_hasRegion = true;
            enterAnnotateMode();
        }
    }
}

void OverlayWindow::activateFullscreen() {
    m_purpose = Screenshot;
    activateInternal();
    // selectedRect() is built from QRect(m_startPos, m_endPos), whose corner
    // points are both INCLUSIVE — (0,0) to (w-1,h-1) is exactly width()×height().
    // Using (w,h) here oversized the selection to (w+1)×(h+1), which made
    // AnnotationCanvas::setRegion request a crop past the image bounds.
    m_startPos = QPoint(0, 0);
    m_endPos = QPoint(width() - 1, height() - 1);
    m_hasRegion = true;
    m_drawing = false;
    enterAnnotateMode();
}

void OverlayWindow::activateForRecording() {
    m_purpose = Record;
    activateInternal();
    // If we restored a remembered region, jump straight to the confirm state
    // with it pre-selected, mirroring the screenshot path's annotate entry.
    if (m_rememberRegion && (m_startPos != m_endPos)) {
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            enterRecordSelectMode();
        }
    }
}

// Persist the just-committed region (window-local) so it survives a restart.
// No-op unless "remember region" is on. Emits lastRegionChanged so main.cpp can
// write it to config.
void OverlayWindow::rememberCurrentRegion() {
    if (!m_rememberRegion) return;
    m_lastStartPos = m_startPos;
    m_lastEndPos = m_endPos;
    emit lastRegionChanged(selectedRect());
}

void OverlayWindow::setLastRegion(const QRect &region) {
    if (region.isValid()) {
        m_lastStartPos = region.topLeft();
        m_lastEndPos = region.bottomRight();
    }
}

void OverlayWindow::handleSystemWake() {
    // A session left open across sleep is anchored to a display layout (and a
    // capture) that no longer exists. Worse, the in-flight SCK completion or
    // its watchdog may never fire once the process has been suspended, so
    // isBusy() would stay true forever and the next hotkey would be silently
    // swallowed as "Capture in progress". Tear everything down here; the next
    // activation captures fresh.
    const bool hadSession = isVisible() || m_captureInFlight || m_hasRegion;
    m_captureInFlight = false;
    m_screenshot = QImage();
    m_drawing = false;
    m_hasRegion = false;
    m_resizing = false;
    m_activeResize = EdgeNone;
    exitAnnotateMode();
    exitRecordSelectMode();
    m_mode = Select;
    hideOverlay();

    // Re-anchor the long-lived native window now that displays are back. A
    // no-op when hidden, but it keeps the next activation off a stale screen.
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (screen) repositionToScreen(screen);

    if (hadSession) qInfo("Overlay: dropped pre-sleep session after wake");
}

QRect OverlayWindow::selectedRect() const {
    return QRect(m_startPos, m_endPos).normalized();
}

OverlayWindow::ResizeEdge OverlayWindow::edgeAt(QPoint pos) const {
    if (!m_hasRegion) return EdgeNone;
    QRect sel = selectedRect();
    const int m = 8;

    // Quick-reject: must be near the rect at all
    QRect outer = sel.adjusted(-m, -m, m, m);
    if (!outer.contains(pos)) return EdgeNone;

    auto near = [&](int v, int target) {
        return std::abs(v - target) <= m;
    };

    bool nL = near(pos.x(), sel.left());
    bool nR = near(pos.x(), sel.right());
    bool nT = near(pos.y(), sel.top());
    bool nB = near(pos.y(), sel.bottom());

    // Corner detection takes priority
    if (nT && nL) return EdgeTopLeft;
    if (nT && nR) return EdgeTopRight;
    if (nB && nL) return EdgeBottomLeft;
    if (nB && nR) return EdgeBottomRight;

    // Edges -- but the perpendicular axis must be within the rect range
    bool xInRange = pos.x() >= sel.left() - m && pos.x() <= sel.right() + m;
    bool yInRange = pos.y() >= sel.top()  - m && pos.y() <= sel.bottom() + m;

    if (nT && xInRange) return EdgeTop;
    if (nB && xInRange) return EdgeBottom;
    if (nL && yInRange) return EdgeLeft;
    if (nR && yInRange) return EdgeRight;

    return EdgeNone;
}

void OverlayWindow::beginResize(ResizeEdge edge) {
    // A half-typed text label is positioned in canvas coords; commit it so it
    // gets translated with the rest instead of floating at a stale spot.
    if (m_mode == Annotate && m_canvas)
        m_canvas->commitTextInput();
    QRect sel = selectedRect();
    m_resizing = true;
    m_activeResize = edge;
    m_resizeFixedLeft   = sel.left();
    m_resizeFixedRight  = sel.right();
    m_resizeFixedTop    = sel.top();
    m_resizeFixedBottom = sel.bottom();
}

void OverlayWindow::applyResize(QPoint pos) {
    // Clamp mouse position to widget bounds
    int mx = std::max(0, std::min(width()  - 1, pos.x()));
    int my = std::max(0, std::min(height() - 1, pos.y()));

    int left   = m_resizeFixedLeft;
    int right  = m_resizeFixedRight;
    int top    = m_resizeFixedTop;
    int bottom = m_resizeFixedBottom;

    switch (m_activeResize) {
        case EdgeTop:         top = my; break;
        case EdgeBottom:      bottom = my; break;
        case EdgeLeft:        left = mx; break;
        case EdgeRight:       right = mx; break;
        case EdgeTopLeft:     top = my; left = mx; break;
        case EdgeTopRight:    top = my; right = mx; break;
        case EdgeBottomLeft:  bottom = my; left = mx; break;
        case EdgeBottomRight: bottom = my; right = mx; break;
        default: break;
    }

    // Min size 5x5 -- keep the fixed side anchored
    if (right - left < 5) {
        if (m_activeResize == EdgeLeft || m_activeResize == EdgeTopLeft || m_activeResize == EdgeBottomLeft) {
            left = right - 5;
        } else if (m_activeResize == EdgeRight || m_activeResize == EdgeTopRight || m_activeResize == EdgeBottomRight) {
            right = left + 5;
        }
    }
    if (bottom - top < 5) {
        if (m_activeResize == EdgeTop || m_activeResize == EdgeTopLeft || m_activeResize == EdgeTopRight) {
            top = bottom - 5;
        } else if (m_activeResize == EdgeBottom || m_activeResize == EdgeBottomLeft || m_activeResize == EdgeBottomRight) {
            bottom = top + 5;
        }
    }

    // Final clamp to widget bounds
    left   = std::max(0, std::min(width()  - 1, left));
    right  = std::max(0, std::min(width()  - 1, right));
    top    = std::max(0, std::min(height() - 1, top));
    bottom = std::max(0, std::min(height() - 1, bottom));

    const QPoint oldTopLeft = selectedRect().topLeft();
    m_startPos = QPoint(left, top);
    m_endPos   = QPoint(right, bottom);

    if (m_mode == Annotate && m_canvas) {
        QRect sel = selectedRect();
        // Annotations are stored in canvas-local coords. Moving the canvas
        // origin (dragging the top/left edge) would otherwise drag every mark
        // along with it; shift them back so they stay on the same pixels.
        const QPoint moved = sel.topLeft() - oldTopLeft;
        if (!moved.isNull())
            m_annotationState.offsetAll(QPointF(-moved));
        m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
        m_canvas->setGeometry(sel);
        if (m_toolbar) {
            m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
        }
    } else if (m_mode == RecordSelect) {
        positionRecordButtons(selectedRect());
    }

    update();
}

void OverlayWindow::enterAnnotateMode() {
    m_mode = Annotate;
    m_hasRegion = true;

    QRect sel = selectedRect();

    // Create canvas over the selected region
    if (!m_canvas) {
        m_canvas = new AnnotationCanvas(&m_annotationState, m_screenshot, this);
        // While a text label is being typed the QLineEdit owns the keyboard,
        // so Save/Copy chords never reached keyPressEvent. Let the canvas
        // hand them to us: commit the text, then act.
        m_canvas->setTextKeyInterceptor([this](QKeyEvent *ev) {
            if (m_bindSave.matches(ev)) { handleSaveAndCopy(); return true; }
            if (m_bindCopy.matches(ev)) { handleCopy(); return true; }
            return false;
        });
    } else {
        // Canvas is reused across activations — refresh its screenshot
        // reference so it crops from the LATEST capture, not the first one.
        m_canvas->setScreenshot(m_screenshot);
    }
    m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
    m_canvas->setGeometry(sel);
    // Child widgets inherit the overlay's cursor. The overlay stops receiving
    // mouse moves once a child is under the pointer, so without an explicit
    // arrow here, moving straight from a resize handle into the canvas leaves
    // the resize cursor stuck until the pointer leaves the selection again.
    m_canvas->setCursor(Qt::ArrowCursor);
    m_canvas->show();
    m_canvas->raise();

    // Create toolbar below the region
    if (!m_toolbar) {
        m_toolbar = new AnnotationToolbar(&m_annotationState, this);
        connect(m_toolbar, &AnnotationToolbar::saveRequested, this, &OverlayWindow::handleSaveAndCopy);
        connect(m_toolbar, &AnnotationToolbar::cancelRequested,
                this, &OverlayWindow::cancelSession);
    }
    m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
    m_toolbar->setCursor(Qt::ArrowCursor);
    m_toolbar->show();
    m_toolbar->raise();

    update();
    syncSelectCursor();
}

void OverlayWindow::exitAnnotateMode() {
    m_mode = Select;
    if (m_canvas) {
        m_canvas->hide();
    }
    if (m_toolbar) {
        m_toolbar->hide();
    }
    syncSelectCursor();
}

// ---- Record-select mode ----

void OverlayWindow::enterRecordSelectMode() {
    m_mode = RecordSelect;
    m_hasRegion = true;

    QRect sel = selectedRect();

    // Create buttons lazily
    if (!m_btnRecordRegion) {
        m_btnRecordRegion = new QPushButton("Record Region (Enter)", this);
        m_btnRecordRegion->setStyleSheet(
            "QPushButton {"
            "  background-color: #ff4444;"
            "  color: white;"
            "  border: none;"
            "  border-radius: 6px;"
            "  padding: 6px 14px;"
            "  font-size: 13px;"
            "  font-weight: 600;"
            "}"
            "QPushButton:hover {"
            "  background-color: #ff2222;"
            "}"
        );
        connect(m_btnRecordRegion, &QPushButton::clicked, this, &OverlayWindow::emitRecordRegion);
    }

    if (!m_btnRecordFullscreen) {
        m_btnRecordFullscreen = new QPushButton("Record Fullscreen", this);
        m_btnRecordFullscreen->setStyleSheet(
            "QPushButton {"
            "  background-color: rgba(255, 68, 68, 50);"
            "  color: #ff6666;"
            "  border: 1px solid rgba(255, 68, 68, 76);"
            "  border-radius: 6px;"
            "  padding: 6px 14px;"
            "  font-size: 13px;"
            "  font-weight: 600;"
            "}"
            "QPushButton:hover {"
            "  background-color: rgba(255, 68, 68, 80);"
            "}"
        );
        connect(m_btnRecordFullscreen, &QPushButton::clicked, this, &OverlayWindow::emitRecordFullscreen);
    }

    if (!m_btnRecordCancel) {
        m_btnRecordCancel = new QPushButton("Cancel (Esc)", this);
        m_btnRecordCancel->setStyleSheet(
            "QPushButton {"
            "  background-color: transparent;"
            "  color: rgba(255, 255, 255, 160);"
            "  border: none;"
            "  padding: 6px 10px;"
            "  font-size: 12px;"
            "}"
            "QPushButton:hover {"
            "  color: rgba(255, 255, 255, 220);"
            "}"
        );
        connect(m_btnRecordCancel, &QPushButton::clicked,
                this, &OverlayWindow::cancelSession);
    }

    // Same inherited-cursor issue as the annotate canvas: these buttons sit
    // right below the region's bottom edge, so leaving a resize handle must
    // not leave the resize cursor stuck over them.
    m_btnRecordRegion->setCursor(Qt::ArrowCursor);
    m_btnRecordFullscreen->setCursor(Qt::ArrowCursor);
    m_btnRecordCancel->setCursor(Qt::ArrowCursor);

    positionRecordButtons(sel);

    update();
    syncSelectCursor();
}

// Lay out (and show) the record-mode action buttons under `sel`. Called on
// enter and again on every edge-resize, so the buttons track the region instead
// of staying anchored under the original rectangle.
void OverlayWindow::positionRecordButtons(const QRect &sel) {
    if (!m_btnRecordRegion || !m_btnRecordFullscreen || !m_btnRecordCancel) return;

    // Adjust button sizes so we can position them
    m_btnRecordRegion->adjustSize();
    m_btnRecordFullscreen->adjustSize();
    m_btnRecordCancel->adjustSize();

    const int gap       = 8;
    const int btnHeight = 32;
    const int yOffset   = 10; // gap below region

    int totalWidth = m_btnRecordRegion->sizeHint().width()
                   + gap
                   + m_btnRecordFullscreen->sizeHint().width()
                   + gap
                   + m_btnRecordCancel->sizeHint().width();

    int startX = sel.x() + sel.width() / 2 - totalWidth / 2;
    int startY = sel.bottom() + yOffset;

    // Clamp so buttons stay within the overlay. startX/startY are WIDGET-LOCAL
    // coords and the overlay spans exactly the cursor's screen, so clamp against
    // our own size. Clamp the far edge FIRST, then the near edge, so a region
    // wider/taller than the overlay floors at 4 instead of going off-screen
    // negative (which the old near-edge-first order allowed).
    const int overlayW = width();
    const int overlayH = height();
    if (startX + totalWidth > overlayW - 4) startX = overlayW - 4 - totalWidth;
    if (startX < 4) startX = 4;
    // No room below the region → place above it; if that's also off-screen
    // (region hugging the top), floor at 4.
    if (startY + btnHeight > overlayH - 4) startY = sel.top() - btnHeight - yOffset;
    if (startY < 4) startY = 4;

    int x = startX;
    m_btnRecordRegion->setGeometry(x, startY, m_btnRecordRegion->sizeHint().width(), btnHeight);
    x += m_btnRecordRegion->sizeHint().width() + gap;
    m_btnRecordFullscreen->setGeometry(x, startY, m_btnRecordFullscreen->sizeHint().width(), btnHeight);
    x += m_btnRecordFullscreen->sizeHint().width() + gap;
    m_btnRecordCancel->setGeometry(x, startY, m_btnRecordCancel->sizeHint().width(), btnHeight);

    m_btnRecordRegion->show();
    m_btnRecordRegion->raise();
    m_btnRecordFullscreen->show();
    m_btnRecordFullscreen->raise();
    m_btnRecordCancel->show();
    m_btnRecordCancel->raise();
}

void OverlayWindow::exitRecordSelectMode() {
    if (m_mode == RecordSelect) {
        m_mode = Select;
    }
    if (m_btnRecordRegion)    m_btnRecordRegion->hide();
    if (m_btnRecordFullscreen) m_btnRecordFullscreen->hide();
    if (m_btnRecordCancel)    m_btnRecordCancel->hide();
    syncSelectCursor();
}

void OverlayWindow::emitRecordRegion() {
    QRect sel = selectedRect();

    // Fix #19: Qt side — bail out on zero/subpixel regions before hitting FFI.
    if (sel.width() < 1 || sel.height() < 1) {
        emit regionInvalid(QStringLiteral("selection is too small"));
        return;
    }

    // Persist before exitRecordSelectMode clears the selection.
    rememberCurrentRegion();

    // sel is in WIDGET-LOCAL coords (0-based), and the overlay is sized to
    // exactly one display, so the region is always within that one display —
    // no cross-display span is possible. Use THIS overlay's screen for the DPR.
    // (The old `screen.geometry().contains(sel)` search compared local coords
    // against GLOBAL screen rects: on a secondary display it matched the primary
    // and applied the primary's DPR, and on a secondary larger than the primary
    // it matched nothing and wrongly rejected the recording.)
    QScreen *selScreen = this->screen();
    double dpr = selScreen ? selScreen->devicePixelRatio()
                           : sf::displayScaleFactor();
    QRect pixelRegion(
        static_cast<int>(sel.x()      * dpr),
        static_cast<int>(sel.y()      * dpr),
        static_cast<int>(sel.width()  * dpr),
        static_cast<int>(sel.height() * dpr)
    );
    exitRecordSelectMode();
    m_hasRegion = false;
    m_drawing   = false;
    hideOverlay();
    emit recordingRequested(m_displayIndex, pixelRegion);
}

void OverlayWindow::emitRecordFullscreen() {
    exitRecordSelectMode();
    m_hasRegion = false;
    m_drawing   = false;
    hideOverlay();
    emit recordingRequested(m_displayIndex, QRect());
}

// ---- end record-select ----

void OverlayWindow::hideOverlay() {
    // Ending the session supersedes any capture still in flight: bump the
    // sequence so its completion/watchdog no-op instead of re-showing a
    // dismissed overlay, and release the busy gate.
    ++m_captureSeq;
    m_captureInFlight = false;
    hide();
    syncSelectCursor();
}

// The crosshair shown while drawing a region. A per-widget setCursor() is not
// reliable here on macOS: with a stationary pointer no mouseMoveEvent fires, and
// any AppKit cursorUpdate (window becoming key, a cursor-rect invalidation, a
// focus change) re-resolves the cursor and can revert the crosshair to the
// default arrow before the user moves — the intermittent "arrow instead of
// crosshair" bug. An application override cursor is immune: setOverrideCursor
// applies synchronously at the call (covering the stationary case) and Qt
// re-applies it on every subsequent cursorUpdate, so no re-resolution can land
// on the arrow. It is held only during the draw-region phase (Select mode, no
// region committed) and released the instant a region exists, so the edge-resize
// cursors in mouseMoveEvent take over normally. Driven purely by state and
// guarded by m_selectCursorActive, so it is safe to call from every transition.
void OverlayWindow::syncSelectCursor() {
    const bool want = isVisible() && m_mode == Select && !m_hasRegion;
    if (want == m_selectCursorActive) return;
    if (want) {
        QGuiApplication::setOverrideCursor(Qt::CrossCursor);
    } else {
        QGuiApplication::restoreOverrideCursor();
    }
    m_selectCursorActive = want;
}

bool OverlayWindow::isBusy() const {
    // The overlay stays hidden until its capture lands (see activateInternal),
    // so a pending capture is busy even while invisible. Bounded: the
    // completion handler or the 8s watchdog always clears it.
    if (m_captureInFlight) return true;
    if (!isVisible()) return false;
    // Visible but idle (no draw in progress, no committed region, default mode,
    // and capture already landed) = stale window AppKit may have hidden behind
    // our back. Allow the hotkey to recover by re-entering activate().
    if (m_drawing || m_hasRegion) return true;
    if (m_mode == Annotate || m_mode == RecordSelect) return true;
    // (A pending capture was handled above. A *failed* capture clears that
    // flag via handleCaptureFailure instead of leaving m_screenshot null
    // forever — the old `m_screenshot.isNull()` test latched busy permanently
    // after any capture failure and killed the hotkey until app restart.)
    return false;
}

void OverlayWindow::handleCaptureFailure(const char *reason) {
    qWarning("Overlay: %s — resetting overlay so the hotkey gate recovers", reason);
    m_captureInFlight = false;
    m_screenshot = QImage();
    m_drawing = false;
    m_hasRegion = false;
    exitAnnotateMode();
    exitRecordSelectMode();
    m_mode = Select;
    hideOverlay();
    emit regionInvalid(
        QStringLiteral("Screen capture failed. If this keeps happening, check "
                       "System Settings → Privacy & Security → Screen "
                       "Recording, then try again."));
}

// Shared Save/Copy precondition. Returns the composited image, or a null
// image (after beeping) if the session can't produce one yet.
QImage OverlayWindow::compositeForCommit() {
    if (!m_canvas) return QImage();
    // Capture hasn't landed yet. The overlay is normally hidden until then,
    // but don't composite a blank frame if we get here anyway — tell the user
    // instead of silently ignoring the action; a retry works once the
    // completion handler refreshes the canvas.
    if (m_captureInFlight || m_screenshot.isNull()) {
        qWarning("Overlay: Save/Copy ignored — capture not ready yet");
        QApplication::beep();
        return QImage();
    }
    // A label still being typed lives only in the QLineEdit — commit it so
    // it makes it into the image (hiding the canvas would discard it).
    m_canvas->commitTextInput();
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) {
        qWarning("Overlay: Save/Copy produced an empty image");
        QApplication::beep();
    }
    return composited;
}

void OverlayWindow::handleCopy() {
    QImage composited = compositeForCommit();
    if (composited.isNull()) return;

    rememberCurrentRegion();
    m_hasRegion = false;
    m_drawing = false;
    exitAnnotateMode();
    hideOverlay();

    emit clipboardReady(composited, composited.width(), composited.height());
}

void OverlayWindow::handleSaveAndCopy() {
    QImage composited = compositeForCommit();
    if (composited.isNull()) return;

    rememberCurrentRegion();
    m_hasRegion = false;
    m_drawing = false;
    exitAnnotateMode();
    hideOverlay();

    // Save only. Whether a save also copies is the "Auto copy to clipboard"
    // preference, which main.cpp's screenshotReady handler applies — emitting
    // clipboardReady here too copied twice and ignored the setting.
    emit screenshotReady(composited, composited.width(), composited.height());
}

void OverlayWindow::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    // Draw pre-captured screenshot as background. Fit to the widget rect so
    // it always covers exactly the overlay regardless of widget size or
    // image's stored dpr. Using rect()-target also avoids stale scale when
    // geometry changes between activations.
    if (!m_screenshot.isNull()) {
        p.drawImage(rect(), m_screenshot);
    }

    if (m_drawing || m_hasRegion) {
        QRect sel = selectedRect();

        // Dim everything outside the selection
        QPainterPath fullPath;
        fullPath.addRect(rect());
        QPainterPath selPath;
        selPath.addRect(sel);
        QPainterPath dimPath = fullPath.subtracted(selPath);
        p.fillPath(dimPath, QColor(0, 0, 0, 100));

        // Selection border -- marching ants effect
        QPen whitePen(Qt::white, 1, Qt::DashLine);
        p.setPen(whitePen);
        p.drawRect(sel);

        QPen darkPen(QColor(0, 0, 0, 150), 1, Qt::DashLine);
        darkPen.setDashOffset(4);
        p.setPen(darkPen);
        p.drawRect(sel);

        // Dimension label, in device pixels (what the saved image will be),
        // not logical points.
        const double scale = captureScale();
        QString label = QString("%1 \u00d7 %2")
                            .arg(qRound(sel.width() * scale))
                            .arg(qRound(sel.height() * scale));
        QFont font("Menlo", 11);
        p.setFont(font);
        QFontMetrics fm(font);
        QRect labelRect = fm.boundingRect(label);
        int lx = sel.x() + sel.width() / 2 - labelRect.width() / 2 - 4;
        int ly = sel.y() - labelRect.height() - 8;
        // Selection touching the top edge: put the label just inside it
        // instead of off-screen. Keep it horizontally on-screen too.
        if (ly < 2) ly = sel.y() + 6;
        lx = std::clamp(lx, 2, std::max(2, width() - labelRect.width() - 10));
        p.fillRect(lx - 2, ly - 1, labelRect.width() + 8, labelRect.height() + 4,
                   QColor(0, 0, 0, 180));
        p.setPen(Qt::white);
        p.drawText(lx + 2, ly + labelRect.height() - 2, label);

        // Resize handles (8 points) -- in Select and Annotate modes (not RecordSelect)
        if (m_hasRegion && !m_drawing && (m_mode == Select || m_mode == Annotate)) {
            QPen handlePen(QColor(0, 0, 0, 200), 1);
            p.setPen(handlePen);
            p.setBrush(Qt::white);
            int hs = 4; // half-size -> 8x8 handles
            QPoint handles[] = {
                sel.topLeft(), sel.topRight(), sel.bottomLeft(), sel.bottomRight(),
                QPoint(sel.center().x(), sel.top()),
                QPoint(sel.center().x(), sel.bottom()),
                QPoint(sel.left(), sel.center().y()),
                QPoint(sel.right(), sel.center().y()),
            };
            for (const auto &pt : handles) {
                p.drawRect(pt.x() - hs, pt.y() - hs, hs * 2, hs * 2);
            }
        }
    } else {
        // No selection yet -- light dim over everything
        p.fillRect(rect(), QColor(0, 0, 0, 40));
    }
}

void OverlayWindow::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        if (m_mode == Annotate) {
            // Edge resize takes priority over click-outside-cancels
            ResizeEdge edge = edgeAt(event->pos());
            if (edge != EdgeNone) {
                beginResize(edge);
                return;
            }

            // Clicking outside the region cancels and hides the overlay
            // so the next hotkey press can re-enter with a fresh capture.
            // Previously this entered a "draw new region" state that left
            // the overlay visible, which the hotkey handler treats as busy
            // (see Fix #14 in main.cpp::hotkeyHandler).
            //
            // Once the user has drawn anything, a stray click outside must not
            // throw the work away: ignore it (Esc / Cancel still cancel).
            QRect sel = selectedRect();
            if (!sel.contains(event->pos())) {
                const bool hasWork = !m_annotationState.annotations().isEmpty()
                                  || m_annotationState.activeAnnotation().has_value()
                                  || (m_canvas && m_canvas->isTextInputActive());
                if (!hasWork)
                    cancelSession();
            }
            // Clicks inside the region are handled by AnnotationCanvas
            return;
        }

        if (m_mode == RecordSelect) {
            // Edge resize takes priority
            ResizeEdge edge = edgeAt(event->pos());
            if (edge != EdgeNone) {
                beginResize(edge);
                return;
            }

            // Clicking outside the region lets user re-draw
            QRect sel = selectedRect();
            if (!sel.contains(event->pos())) {
                exitRecordSelectMode();
                m_startPos = event->pos();
                m_endPos = event->pos();
                m_drawing = true;
                m_hasRegion = false;
                update();
                syncSelectCursor();
            }
            return;
        }

        // Select mode never holds a committed region (every commit path moves
        // to Annotate/RecordSelect, every exit path clears m_hasRegion), so a
        // press here always starts a fresh rubber-band.
        m_startPos = event->pos();
        m_endPos = event->pos();
        m_drawing = true;
        m_hasRegion = false;
        update();
        syncSelectCursor();
    }
}

void OverlayWindow::mouseMoveEvent(QMouseEvent *event) {
    if (m_resizing) {
        applyResize(event->pos());
        return;
    }

    if (m_drawing) {
        // Clamp: a drag past the screen edge must not produce a selection
        // (and crop) outside the captured frame.
        m_endPos = clampToWidget(event->pos());
        update();
        return;
    }

    // Update cursor based on edge proximity
    if (m_hasRegion && (m_mode == Select || m_mode == Annotate || m_mode == RecordSelect)) {
        ResizeEdge edge = edgeAt(event->pos());
        switch (edge) {
            case EdgeTopLeft:
            case EdgeBottomRight:
                setCursor(Qt::SizeFDiagCursor);
                break;
            case EdgeTopRight:
            case EdgeBottomLeft:
                setCursor(Qt::SizeBDiagCursor);
                break;
            case EdgeTop:
            case EdgeBottom:
                setCursor(Qt::SizeVerCursor);
                break;
            case EdgeLeft:
            case EdgeRight:
                setCursor(Qt::SizeHorCursor);
                break;
            case EdgeNone:
            default:
                // This branch is guarded by m_hasRegion above, so the old
                // "Select && !m_hasRegion → crosshair" case was unreachable;
                // the crosshair path lives in the else-if below.
                setCursor(Qt::ArrowCursor);
                break;
        }
    } else if (m_mode == Select) {
        // Pre-draw hover in select mode: AppKit resets to arrow on move, so set
        // the native crosshair directly rather than relying on the (stale on
        // this window) override/widget cursor. See setNativeCrosshair().
        setNativeCrosshair();
    }
}

void OverlayWindow::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton && m_resizing) {
        m_resizing = false;
        m_activeResize = EdgeNone;
        if (m_mode == Annotate && m_canvas) {
            QRect sel = selectedRect();
            m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
            m_canvas->setGeometry(sel);
            if (m_toolbar) {
                m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
            }
        } else if (m_mode == RecordSelect) {
            positionRecordButtons(selectedRect());
        }
        update();
        return;
    }

    if (event->button() == Qt::LeftButton && m_drawing) {
        m_drawing = false;
        m_endPos = clampToWidget(event->pos());
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            if (m_purpose == Record) {
                enterRecordSelectMode();
            } else {
                enterAnnotateMode();
            }
        }
        // Region committed (enter*Mode pops the override) or too small to commit
        // (still in the draw phase, keep the crosshair). Either way reconcile.
        syncSelectCursor();
    }
}

void OverlayWindow::keyPressEvent(QKeyEvent *event) {
    // Escape ALWAYS cancels, whatever the user bound "Cancel" to — it's the
    // universal way out of a full-screen overlay. (While a text label is
    // being typed the canvas consumes Escape to discard the label first.)
    const bool isCancel = event->key() == Qt::Key_Escape || m_bindCancel.matches(event);
    const bool isForceQuit = m_bindForceQuit.matches(event);
    if (isForceQuit) {
        QCoreApplication::quit();
        return;
    }

    if (m_mode == Annotate) {
        // If text input is active in the canvas, let it handle keys (Save /
        // Copy chords are forwarded via the canvas's key interceptor).
        if (m_canvas && m_canvas->isTextInputActive()) {
            if (m_bindSave.matches(event)) { handleSaveAndCopy(); return; }
            if (m_bindCopy.matches(event)) { handleCopy(); return; }
            QWidget::keyPressEvent(event);
            return;
        }

        // All chords below come from the Preferences Hotkeys tab (see
        // reloadKeyBindings); the comments give the defaults.

        // Redo / undo (defaults Cmd+Shift+Z / Cmd+Z)
        if (m_bindRedo.matches(event)) { m_annotationState.redo(); return; }
        if (m_bindUndo.matches(event)) { m_annotationState.undo(); return; }

        // Save and copy (default Cmd+S)
        if (m_bindSave.matches(event)) {
            handleSaveAndCopy();
            return;
        }

        // Copy only (default Cmd+C)
        if (m_bindCopy.matches(event)) {
            handleCopy();
            return;
        }

        // Cancel (Escape, or the user's Cancel binding): exit and hide
        if (isCancel) {
            cancelSession();
            return;
        }

        // Stroke width shortcuts (defaults 1/2/3)
        if (m_bindSizeSmall.matches(event))  { m_annotationState.setStrokeWidth(1); return; }
        if (m_bindSizeMedium.matches(event)) { m_annotationState.setStrokeWidth(2); return; }
        if (m_bindSizeLarge.matches(event))  { m_annotationState.setStrokeWidth(4); return; }

        // Tool shortcuts (defaults A/R/C/L/D/F/T/H/B/N/I/M, modifier-exact)
        for (const auto &tb : m_toolBindings) {
            if (tb.first.matches(event)) {
                m_annotationState.setActiveTool(tb.second);
                return;
            }
        }

        return;
    }

    if (m_mode == RecordSelect) {
        if (event->key() == Qt::Key_Return || event->key() == Qt::Key_Enter) {
            emitRecordRegion();
            return;
        }
        // Cancel (Escape, or the user's Cancel binding)
        if (isCancel) {
            cancelSession();
            return;
        }
        return;
    }

    // Select mode (drawing / about to draw a region). There is no committed
    // region in this mode — enter*Mode() runs on mouse release — so the only
    // key that does anything is cancel. (The old Return / Cmd+C branches here
    // were gated on m_hasRegion and could never run.)
    if (isCancel)
        cancelSession();
}

QPoint OverlayWindow::clampToWidget(QPoint p) const {
    return QPoint(std::clamp(p.x(), 0, std::max(0, width() - 1)),
                  std::clamp(p.y(), 0, std::max(0, height() - 1)));
}

// Device pixels per logical point of the current capture. Derived from the
// captured frame itself (its width over the overlay's width) rather than
// QScreen::devicePixelRatio(): they normally agree, but if they don't (scaled
// display modes, a reconfiguration between capture and paint) the frame is
// the ground truth for every crop.
double OverlayWindow::captureScale() const {
    const double screenDpr = screen() ? screen()->devicePixelRatio() : 0.0;
    if (!m_screenshot.isNull() && width() > 0)
        return double(m_screenshot.width()) / double(width());
    if (screenDpr > 0.0) return screenDpr;
    const double f = sf::displayScaleFactor();
    return f > 0.0 ? f : 1.0;
}
