#include "OverlayWindow.h"
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QScreen>
#include <QGuiApplication>
#include <QElapsedTimer>
#include <QFontMetrics>
#include "snapforge_ffi.h"

// --- CaptureWorker: runs SCK capture off the main thread ---

void CaptureWorker::run() {
    CapturedImage img = snapforge_capture_fullscreen(0);
    if (img.data && img.width > 0) {
        QImage qimg(img.data, img.width, img.height, img.width * 4,
                    QImage::Format_RGBA8888);
        QImage copy = qimg.copy(); // deep copy before freeing
        snapforge_free_buffer(img.data, img.len);
        emit captured(copy);
    }
}

// --- OverlayWindow ---

OverlayWindow::OverlayWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_ShowWithoutActivating, false);
    setCursor(Qt::CrossCursor);
    setMouseTracking(true);

    // Pre-warm font metrics to avoid 125ms alias scan on first paint
    QFont font("Menlo", 11);
    QFontMetrics fm(font);
    fm.boundingRect("0");

    // When capture completes on bg thread, update the screenshot and repaint
    connect(&m_captureWorker, &CaptureWorker::captured, this, [this](QImage image) {
        m_screenshot = image;
        update();
    });
}

void OverlayWindow::activate() {
    QElapsedTimer timer;
    timer.start();

    // Size to primary screen
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        setGeometry(screen->geometry());
    }

    m_drawing = false;
    m_hasRegion = false;
    m_startPos = QPoint();
    m_endPos = QPoint();
    m_screenshot = QImage(); // clear old screenshot

    // Show overlay IMMEDIATELY — don't wait for capture
    showFullScreen();
    activateWindow();
    raise();

    qDebug("Overlay shown in %lld ms", timer.elapsed());

    // Kick off capture in background — screenshot will appear when ready
    if (!m_captureWorker.isRunning()) {
        m_captureWorker.start();
    }
}

QRect OverlayWindow::selectedRect() const {
    return QRect(m_startPos, m_endPos).normalized();
}

void OverlayWindow::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    // Draw pre-captured screenshot as background (if available)
    if (!m_screenshot.isNull()) {
        p.drawImage(0, 0, m_screenshot.scaled(size(), Qt::IgnoreAspectRatio, Qt::FastTransformation));
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

        // Selection border — marching ants effect
        QPen whitePen(Qt::white, 1, Qt::DashLine);
        p.setPen(whitePen);
        p.drawRect(sel);

        QPen darkPen(QColor(0, 0, 0, 150), 1, Qt::DashLine);
        darkPen.setDashOffset(4);
        p.setPen(darkPen);
        p.drawRect(sel);

        // Dimension label
        QString label = QString("%1 × %2").arg(sel.width()).arg(sel.height());
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

        // Resize handles (8 points)
        if (m_hasRegion && !m_drawing) {
            p.setPen(QColor(0, 0, 0, 128));
            p.setBrush(Qt::white);
            int hs = 4; // half-size
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
        // No selection yet — light dim over everything
        p.fillRect(rect(), QColor(0, 0, 0, 40));
    }
}

void OverlayWindow::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        m_startPos = event->pos();
        m_endPos = event->pos();
        m_drawing = true;
        m_hasRegion = false;
        update();
    }
}

void OverlayWindow::mouseMoveEvent(QMouseEvent *event) {
    if (m_drawing) {
        m_endPos = event->pos();
        update();
    }
}

void OverlayWindow::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton && m_drawing) {
        m_drawing = false;
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            m_hasRegion = true;
            update();
        }
    }
}

void OverlayWindow::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_Escape) {
        m_hasRegion = false;
        m_drawing = false;
        hide();
        emit cancelled();
    } else if (event->key() == Qt::Key_Return && m_hasRegion) {
        QRect sel = selectedRect();
        hide();

        // Get DPI scale factor for pixel-accurate capture
        double dpr = snapforge_display_scale_factor();
        int px = static_cast<int>(sel.x() * dpr);
        int py = static_cast<int>(sel.y() * dpr);
        int pw = static_cast<int>(sel.width() * dpr);
        int ph = static_cast<int>(sel.height() * dpr);

        emit regionCaptured(px, py, pw, ph);
    } else if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_C && m_hasRegion) {
        // Cmd+C / Ctrl+C — copy to clipboard
        QRect sel = selectedRect();
        double dpr = snapforge_display_scale_factor();
        int px = static_cast<int>(sel.x() * dpr);
        int py = static_cast<int>(sel.y() * dpr);
        int pw = static_cast<int>(sel.width() * dpr);
        int ph = static_cast<int>(sel.height() * dpr);

        CapturedImage img = snapforge_capture_region(0, px, py, pw, ph);
        if (img.data && img.width > 0) {
            snapforge_copy_to_clipboard(img.data, img.width, img.height);
            snapforge_free_buffer(img.data, img.len);
        }

        m_hasRegion = false;
        m_drawing = false;
        hide();
        emit cancelled();
    }
}
