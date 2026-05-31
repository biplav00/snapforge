#include "OverlayWindow.h"
#include "AnnotationCanvas.h"
#include "AnnotationToolbar.h"
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QScreen>
#include <QGuiApplication>
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
#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <objc/runtime.h>
#include <objc/message.h>
#endif

// --- Tool shortcut map ---

const QMap<int, ToolType> &OverlayWindow::toolShortcuts() {
    static const QMap<int, ToolType> map = {
        { Qt::Key_A, ToolType::Arrow },
        { Qt::Key_R, ToolType::Rect },
        { Qt::Key_C, ToolType::Circle },
        { Qt::Key_L, ToolType::Line },
        { Qt::Key_D, ToolType::DottedLine },
        { Qt::Key_F, ToolType::Freehand },
        { Qt::Key_T, ToolType::Text },
        { Qt::Key_H, ToolType::Highlight },
        { Qt::Key_B, ToolType::Blur },
        { Qt::Key_N, ToolType::Steps },
        { Qt::Key_I, ToolType::ColorPicker },
        { Qt::Key_M, ToolType::Measure },
    };
    return map;
}

// --- OverlayWindow ---

OverlayWindow::OverlayWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool)
{
    setCursor(Qt::CrossCursor);
    setMouseTracking(true);

    // Pre-warm font metrics to avoid 125ms alias scan on first paint
    QFont font("Menlo", 11);
    QFontMetrics fm(font);
    fm.boundingRect("0");
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
        int display = snapforge_display_at_point(cursor.x(), cursor.y());
        m_displayIndex = (display >= 0) ? display : 0;
    }

    // Always clear previous capture so a failed fresh capture can't
    // silently reuse the last activation's image (which would show the
    // wrong window when switching apps between screenshots).
    m_screenshot = QImage();
    m_captureInFlight = true;
    const quint64 captureSeq = ++m_captureSeq;

    // Size overlay to the cursor's screen (not primary).
    if (screen) {
        setGeometry(screen->geometry());
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
        self->update();
    });
    QFuture<QImage> future = QtConcurrent::run([displayIndex]() -> QImage {
        CapturedImage img = snapforge_capture_fullscreen(displayIndex);
        if (!img.data || img.width == 0) {
            return QImage();
        }
        QImage qimg(img.data, img.width, img.height, img.width * 4,
                    QImage::Format_RGBA8888);
        QImage copy = qimg.copy();
        snapforge_free_buffer(img.data, img.len);
        return copy;
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
    // Use the full widget rect (width(), height()) — selectedRect() is built
    // from QRect(m_startPos, m_endPos) which is inclusive on both ends, so
    // (0,0) to (w,h) covers the entire pixel range. The previous (w-1, h-1)
    // dropped the rightmost column and bottom row.
    m_startPos = QPoint(0, 0);
    m_endPos = QPoint(width(), height());
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
}

void OverlayWindow::exitAnnotateMode() {
    m_mode = Select;
    if (m_canvas) {
        m_canvas->hide();
    }
    if (m_toolbar) {
        m_toolbar->hide();
    }
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

    // Clamp so buttons stay on screen
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        int screenW = screen->geometry().width();
        int screenH = screen->geometry().height();
        if (startX < 4) startX = 4;
        if (startX + totalWidth > screenW - 4) startX = screenW - 4 - totalWidth;
        if (startY + btnHeight > screenH - 4) startY = sel.top() - btnHeight - yOffset;
    }

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
}

void OverlayWindow::exitRecordSelectMode() {
    if (m_mode == RecordSelect) {
        m_mode = Select;
    }
    if (m_btnRecordRegion)    m_btnRecordRegion->hide();
    if (m_btnRecordFullscreen) m_btnRecordFullscreen->hide();
    if (m_btnRecordCancel)    m_btnRecordCancel->hide();
}

void OverlayWindow::emitRecordRegion() {
    QRect sel = selectedRect();

    // Fix #19: Qt side — bail out on zero/subpixel regions before hitting FFI.
    if (sel.width() < 1 || sel.height() < 1) {
        emit regionInvalid(QStringLiteral("selection is too small"));
        return;
    }

    // Fix #11: refuse selections that span multiple displays. Capture backend
    // is single-display only, so a multi-monitor rect would silently clip.
    const QList<QScreen *> screens = QGuiApplication::screens();
    bool contained = false;
    QScreen *selScreen = nullptr;
    for (QScreen *s : screens) {
        if (s->geometry().contains(sel)) {
            contained = true;
            selScreen = s;
            break;
        }
    }
    if (!contained) {
        emit regionInvalid(QStringLiteral("selection must be on one display"));
        return;
    }

    // Fix #12: use the DPR of the display that actually contains the region,
    // not the primary's.
    double dpr = selScreen ? selScreen->devicePixelRatio()
                           : snapforge_display_scale_factor();
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
    }
}

void OverlayWindow::keyPressEvent(QKeyEvent *event) {
    if (m_mode == Annotate) {
        // If text input is active in the canvas, let it handle keys
        if (m_canvas && m_canvas->isTextInputActive()) {
            QWidget::keyPressEvent(event);
            return;
        }

        const auto modifiers = event->modifiers();
        const bool cmd = modifiers & Qt::ControlModifier;
        const bool shift = modifiers & Qt::ShiftModifier;

        // Cmd+Z / Cmd+Shift+Z: undo/redo
        if (cmd && event->key() == Qt::Key_Z) {
            if (shift) {
                m_annotationState.redo();
            } else {
                m_annotationState.undo();
            }
            return;
        }

        // Cmd+S: save and copy
        if (cmd && event->key() == Qt::Key_S) {
            handleSaveAndCopy();
            return;
        }

        // Cmd+C: copy only
        if (cmd && event->key() == Qt::Key_C) {
            handleCopy();
            return;
        }

        // Escape: exit annotate mode and hide
        if (event->key() == Qt::Key_Escape) {
            m_annotationState.clearAnnotations();
            exitAnnotateMode();
            m_hasRegion = false;
            m_drawing = false;
            hideOverlay();
            emit cancelled();
            return;
        }

        // Stroke width shortcuts: 1/2/3
        if (event->key() == Qt::Key_1) { m_annotationState.setStrokeWidth(1); return; }
        if (event->key() == Qt::Key_2) { m_annotationState.setStrokeWidth(2); return; }
        if (event->key() == Qt::Key_3) { m_annotationState.setStrokeWidth(4); return; }

        // Tool shortcuts (only when no modifier)
        if (!cmd && !shift) {
            const auto &shortcuts = toolShortcuts();
            auto it = shortcuts.find(event->key());
            if (it != shortcuts.end()) {
                m_annotationState.setActiveTool(it.value());
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
        if (event->key() == Qt::Key_Escape) {
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
    if (event->key() == Qt::Key_Escape) {
        m_hasRegion = false;
        m_drawing = false;
        hideOverlay();
        emit cancelled();
    } else if (event->key() == Qt::Key_Return && m_hasRegion) {
        enterAnnotateMode();
    } else if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_C && m_hasRegion) {
        // Cmd+C / Ctrl+C -- copy to clipboard
        QRect sel = selectedRect();

        // Fix #19: ignore zero-size regions.
        if (sel.width() < 2 || sel.height() < 2) {
            emit regionInvalid(QStringLiteral("selection is too small"));
            return;
        }

        // Fix #12: use the DPR of the display containing the region.
        QScreen *selScreen = nullptr;
        for (QScreen *s : QGuiApplication::screens()) {
            if (s->geometry().contains(sel)) { selScreen = s; break; }
        }
        double dpr = selScreen ? selScreen->devicePixelRatio()
                               : snapforge_display_scale_factor();
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
            CapturedImage img = snapforge_capture_region(displayIndex, px, py, pw, ph);
            if (img.data && img.width > 0) {
                QJsonObject req;
                req[QStringLiteral("copy_to_clipboard")] = true;
                QByteArray reqBytes = QJsonDocument(req).toJson(QJsonDocument::Compact);
                char *resJson = snapforge_save_prerendered(img.data, img.len,
                                                           img.width, img.height,
                                                           reqBytes.constData());
                if (resJson) {
                    snapforge_free_string(resJson);
                } else if (char *err = snapforge_app_last_error()) {
                    qWarning("Cmd+C clipboard copy failed: %s", err);
                    snapforge_free_string(err);
                }
                snapforge_free_buffer(img.data, img.len);
            }
        });
    }
}
