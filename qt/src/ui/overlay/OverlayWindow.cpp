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
#include <QThreadPool>
#include <QJsonDocument>
#include <QJsonObject>
#include <algorithm>
#include <cmath>
#include "SnapforgeClient.h"
#include "Shortcuts.h"

#ifdef Q_OS_MAC
#include <objc/runtime.h>
#include <objc/message.h>
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
    return key == Qt::Key_Return && event->key() == Qt::Key_Enter;
}

OverlayWindow::KeyBinding OverlayWindow::bindingFor(const char *section,
                                                    const char *actionId,
                                                    const char *defaultChord) {
    KeyBinding b;
    const QString c = shortcuts::chord(QLatin1String(section),
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
    m_toolBindings.clear();
    for (const auto &t : kTools)
        m_toolBindings.append(qMakePair(bindingFor("tools", t.id, t.def), t.tool));

    m_bindSizeSmall  = bindingFor("sizes", "small",  "1");
    m_bindSizeMedium = bindingFor("sizes", "medium", "2");
    m_bindSizeLarge  = bindingFor("sizes", "large",  "3");

    m_bindSave   = bindingFor("actions", "save",   "Cmd+S");
    m_bindCopy   = bindingFor("actions", "copy",   "Cmd+C");
    m_bindUndo   = bindingFor("actions", "undo",   "Cmd+Z");
    m_bindRedo   = bindingFor("actions", "redo",   "Cmd+Shift+Z");
    m_bindCancel = bindingFor("actions", "cancel", "Escape");
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
    // Topology changed underneath us. If the overlay is hidden, do nothing: the
    // next activate() repositions to the cursor's screen from scratch. If it's
    // visible, re-anchor now so it follows the new layout instead of lingering
    // on a screen that may have moved or vanished.
    if (!isVisible())
        return;
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();
    repositionToScreen(screen);
}

// Shared internal activation logic
void OverlayWindow::activateInternal() {
    QElapsedTimer timer;
    timer.start();

    m_drawing = false;
    m_hasRegion = false;
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

    // Show the overlay BEFORE capturing. Previously snapforge_capture_fullscreen
    // ran synchronously and could block the UI thread for several seconds when
    // SCK was slow to deliver the first frame. We now show the dimmed overlay
    // immediately and swap in the screenshot when the worker completes; until
    // then, paintEvent draws light dim over the real desktop.
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
        // Discard stale completions: user dismissed overlay or switched display
        // (Space change) before capture finished. Without these guards we'd
        // paint the prior Space's screenshot into the current overlay.
        if (!self->isVisible()) return;
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
        self->update();
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

    // The region-select crosshair is applied by syncSelectCursor() at the end of
    // this function (and by every later state transition). It uses an application
    // override cursor rather than a per-widget setCursor() because the latter is
    // not reliable on macOS for a stationary pointer — see syncSelectCursor().

    show();
    activateWindow();
    raise();

#ifdef Q_OS_MAC
    {
        auto *nsView = reinterpret_cast<id>(winId());
        id nsWindow = ((id (*)(id, SEL))objc_msgSend)(nsView, sel_registerName("window"));
        if (nsWindow) {
            ((void (*)(id, SEL, long))objc_msgSend)(nsWindow, sel_registerName("setLevel:"), 1000);
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

    // Apply the draw-region crosshair now that the window is shown and (on macOS)
    // key. Uses an application override cursor so it survives the AppKit cursor
    // re-evaluations that a stationary pointer would otherwise lose it to.
    syncSelectCursor();

    qDebug("Overlay shown in %lld ms", timer.elapsed());
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

    m_startPos = QPoint(left, top);
    m_endPos   = QPoint(right, bottom);

    if (m_mode == Annotate && m_canvas) {
        QRect sel = selectedRect();
        m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
        m_canvas->setGeometry(sel);
        if (m_toolbar) {
            m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
        }
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
    } else {
        // Canvas is reused across activations — refresh its screenshot
        // reference so it crops from the LATEST capture, not the first one.
        m_canvas->setScreenshot(m_screenshot);
    }
    m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
    m_canvas->setGeometry(sel);
    m_canvas->show();
    m_canvas->raise();

    // Create toolbar below the region
    if (!m_toolbar) {
        m_toolbar = new AnnotationToolbar(&m_annotationState, this);
        connect(m_toolbar, &AnnotationToolbar::saveRequested, this, &OverlayWindow::handleSaveAndCopy);
        connect(m_toolbar, &AnnotationToolbar::copyRequested, this, &OverlayWindow::handleCopy);
        connect(m_toolbar, &AnnotationToolbar::cancelRequested, this, [this]() {
            exitAnnotateMode();
            m_hasRegion = false;
            m_drawing = false;
            hideOverlay();
            emit cancelled();
        });
    }
    m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
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
        connect(m_btnRecordCancel, &QPushButton::clicked, this, [this]() {
            exitRecordSelectMode();
            m_hasRegion = false;
            m_drawing = false;
            hideOverlay();
            emit cancelled();
        });
    }

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
    // coords and the overlay spans exactly the cursor's screen, so clamp
    // against our own size — the primary screen's geometry is the wrong bound
    // on a secondary display with a different resolution.
    const int overlayW = width();
    const int overlayH = height();
    if (startX < 4) startX = 4;
    if (startX + totalWidth > overlayW - 4) startX = overlayW - 4 - totalWidth;
    if (startY + btnHeight > overlayH - 4) startY = sel.top() - btnHeight - yOffset;

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

    update();
    syncSelectCursor();
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
    if (!isVisible()) return false;
    // Visible but idle (no draw in progress, no committed region, default mode,
    // and capture already landed) = stale window AppKit may have hidden behind
    // our back. Allow the hotkey to recover by re-entering activate().
    if (m_drawing || m_hasRegion) return true;
    if (m_mode == Annotate || m_mode == RecordSelect) return true;
    // Busy only while a capture is genuinely pending. A *failed* capture clears
    // this flag (via handleCaptureFailure) instead of leaving m_screenshot null
    // forever — the old `m_screenshot.isNull()` test latched busy permanently
    // after any capture failure and killed the hotkey until app restart.
    if (m_captureInFlight) return true;
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

void OverlayWindow::handleSave() {
    if (!m_canvas) return;
    // Capture hasn't landed yet (user hit save while the async fullscreen grab
    // was still in flight — possible via remembered-region/fullscreen, which
    // enter annotate mode before the screenshot resolves). compositeImage()
    // would be blank. Ignore the save and keep the overlay up; the completion
    // handler refreshes the canvas, so a second save works.
    if (m_captureInFlight) return;
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) return;

    if (m_rememberRegion) { m_lastStartPos = m_startPos; m_lastEndPos = m_endPos; }
    m_hasRegion = false;
    m_drawing = false;
    exitAnnotateMode();
    hideOverlay();

    emit screenshotReady(composited, composited.width(), composited.height());
}

void OverlayWindow::handleCopy() {
    if (!m_canvas) return;
    // See handleSave: skip while the async capture is still pending so a blank
    // composite isn't copied; retry works once the canvas refreshes.
    if (m_captureInFlight) return;
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) return;

    if (m_rememberRegion) { m_lastStartPos = m_startPos; m_lastEndPos = m_endPos; }
    m_hasRegion = false;
    m_drawing = false;
    exitAnnotateMode();
    hideOverlay();

    emit clipboardReady(composited, composited.width(), composited.height());
}

void OverlayWindow::handleSaveAndCopy() {
    if (!m_canvas) return;
    // See handleSave: skip while the async capture is still pending so a blank
    // composite isn't saved/copied; retry works once the canvas refreshes.
    if (m_captureInFlight) return;
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) return;

    if (m_rememberRegion) { m_lastStartPos = m_startPos; m_lastEndPos = m_endPos; }
    m_hasRegion = false;
    m_drawing = false;
    exitAnnotateMode();
    hideOverlay();

    emit screenshotReady(composited, composited.width(), composited.height());
    emit clipboardReady(composited, composited.width(), composited.height());
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

        // Dimension label
        QString label = QString("%1 \u00d7 %2").arg(sel.width()).arg(sel.height());
        QFont font("Menlo", 11);
        p.setFont(font);
        QFontMetrics fm(font);
        QRect labelRect = fm.boundingRect(label);
        int lx = sel.x() + sel.width() / 2 - labelRect.width() / 2 - 4;
        int ly = sel.y() - labelRect.height() - 8;
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
            QRect sel = selectedRect();
            if (!sel.contains(event->pos())) {
                m_annotationState.clearAnnotations();
                exitAnnotateMode();
                m_hasRegion = false;
                m_drawing = false;
                hideOverlay();
                emit cancelled();
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

        // Select mode: edge resize when we already have a region
        if (m_hasRegion && !m_drawing && m_mode == Select) {
            ResizeEdge edge = edgeAt(event->pos());
            if (edge != EdgeNone) {
                beginResize(edge);
                return;
            }
        }

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
        m_endPos = event->pos();
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
                if (m_mode == Select && !m_hasRegion) {
                    setCursor(Qt::CrossCursor);
                } else {
                    setCursor(Qt::ArrowCursor);
                }
                break;
        }
    } else if (m_mode == Select) {
        setCursor(Qt::CrossCursor);
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
        }
        update();
        return;
    }

    if (event->button() == Qt::LeftButton && m_drawing) {
        m_drawing = false;
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
    if (m_mode == Annotate) {
        // If text input is active in the canvas, let it handle keys
        if (m_canvas && m_canvas->isTextInputActive()) {
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

        // Cancel (default Escape): exit annotate mode and hide
        if (m_bindCancel.matches(event)) {
            m_annotationState.clearAnnotations();
            exitAnnotateMode();
            m_hasRegion = false;
            m_drawing = false;
            hideOverlay();
            emit cancelled();
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
        // Cancel (default Escape)
        if (m_bindCancel.matches(event)) {
            exitRecordSelectMode();
            m_hasRegion = false;
            m_drawing = false;
            hideOverlay();
            emit cancelled();
            return;
        }
        return;
    }

    // Select mode
    if (m_bindCancel.matches(event)) {
        // Cancel (default Escape)
        m_hasRegion = false;
        m_drawing = false;
        hideOverlay();
        emit cancelled();
    } else if (event->key() == Qt::Key_Return && m_hasRegion) {
        enterAnnotateMode();
    } else if (m_bindCopy.matches(event) && m_hasRegion) {
        // Copy to clipboard (default Cmd+C)
        QRect sel = selectedRect();

        // Fix #19: ignore zero-size regions.
        if (sel.width() < 2 || sel.height() < 2) {
            emit regionInvalid(QStringLiteral("selection is too small"));
            return;
        }

        // sel is widget-local and the overlay covers exactly one display, so
        // the region's display is THIS overlay's screen. (The old
        // geometry().contains(sel) search compared local coords to global rects
        // and picked the wrong screen's DPR on multi-monitor setups.)
        QScreen *selScreen = this->screen();
        double dpr = selScreen ? selScreen->devicePixelRatio()
                               : sf::displayScaleFactor();
        int px = static_cast<int>(sel.x() * dpr);
        int py = static_cast<int>(sel.y() * dpr);
        int pw = static_cast<int>(sel.width() * dpr);
        int ph = static_cast<int>(sel.height() * dpr);

        // Hide overlay first, then capture+copy on a worker so SCK latency
        // doesn't block the UI thread. By the time the worker captures, the
        // overlay is gone and the underlying desktop is what we want anyway.
        int displayIndex = m_displayIndex;
        m_hasRegion = false;
        m_drawing = false;
        hideOverlay();
        emit cancelled();
        QThreadPool::globalInstance()->start([displayIndex, px, py, pw, ph]() {
            // Raw capture stays on the primitive (snapforge_capture_region):
            // this is the unmodified desktop bitmap with no annotations to
            // preserve. We pipe it through the use-case clipboard path so
            // the clipboard write goes via snapforge_save_prerendered
            // (output_path omitted = clipboard-only).
            auto captured = sf::captureRegion(
                static_cast<uint32_t>(displayIndex),
                sf::Region{px, py, static_cast<uint32_t>(pw), static_cast<uint32_t>(ph)});
            if (captured) {
                const QImage &img = *captured;
                sf::SaveReq req;            // outputPath empty = clipboard-only
                req.copyToClipboard = true;
                if (!sf::savePrerendered(img.constBits(),
                                         static_cast<size_t>(img.sizeInBytes()),
                                         static_cast<uint32_t>(img.width()),
                                         static_cast<uint32_t>(img.height()),
                                         req)) {
                    const QString err = sf::lastError();
                    if (!err.isEmpty())
                        qWarning("Cmd+C clipboard copy failed: %s", qUtf8Printable(err));
                }
            }
        });
    }
}
