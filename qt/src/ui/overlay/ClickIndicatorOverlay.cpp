#include "ClickIndicatorOverlay.h"

#include <QPainter>
#include <QPainterPath>
#include <QPaintEvent>
#include <QScreen>
#include <QGuiApplication>
#include <QTimer>
#include <QWindow>
#include <algorithm>
#include <utility>

#ifdef Q_OS_MACOS
// Implemented in ClickIndicatorOverlayMac.mm — keeps AppKit out of this
// translation unit so it can stay a plain .cpp.
void clickoverlay_configure_macos_window(QWindow *w);
#endif

// ---------------------------------------------------------------------------
// ClickRippleWindow — one transparent overlay per display
// ---------------------------------------------------------------------------

ClickRippleWindow::ClickRippleWindow(QScreen *screen, QWidget *parent)
    : QWidget(parent,
              Qt::FramelessWindowHint
              | Qt::WindowStaysOnTopHint
              | Qt::Tool
              | Qt::WindowDoesNotAcceptFocus
              | Qt::BypassWindowManagerHint)
    , m_screen(screen)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_TransparentForMouseEvents);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setFocusPolicy(Qt::NoFocus);

    m_anim = new QTimer(this);
    m_anim->setInterval(16);  // ~60fps
    connect(m_anim, &QTimer::timeout, this, &ClickRippleWindow::tick);

    m_clock.start();
}

ClickRippleWindow::~ClickRippleWindow() = default;

void ClickRippleWindow::showOverlay()
{
    // Cover exactly our display (re-read every show: the screen may have
    // been repositioned or changed resolution since the last recording).
    if (m_screen) {
        const QRect geo = m_screen->geometry();
        if (!geo.isEmpty()) setGeometry(geo);
    }
    show();

#ifdef Q_OS_MACOS
    if (QWindow *w = windowHandle())
        clickoverlay_configure_macos_window(w);
#endif

    raise();
}

void ClickRippleWindow::hideOverlay()
{
    m_anim->stop();
    m_ripples.clear();
    hide();
}

void ClickRippleWindow::addRipple(QPoint globalPos, bool rightClick)
{
    if (!isVisible()) return;
    Ripple r;
    r.localPos = QPointF(globalPos - geometry().topLeft());
    r.startMs = m_clock.elapsed();
    r.rightClick = rightClick;
    m_ripples.push_back(r);
    if (!m_anim->isActive()) m_anim->start();
    update();
}

void ClickRippleWindow::tick()
{
    const qint64 now = m_clock.elapsed();
    m_ripples.erase(
        std::remove_if(m_ripples.begin(), m_ripples.end(),
                       [now](const Ripple &r) {
                           return (now - r.startMs) >= kLifetimeMs;
                       }),
        m_ripples.end());
    if (m_ripples.isEmpty()) m_anim->stop();
    update();
}

void ClickRippleWindow::paintEvent(QPaintEvent *)
{
    if (m_ripples.isEmpty()) return;

    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    const qint64 now = m_clock.elapsed();
    for (const Ripple &r : m_ripples) {
        const qreal t = qBound(0.0,
                               (now - r.startMs) / static_cast<qreal>(kLifetimeMs),
                               1.0);
        const qreal radius = kStartRadius + (kEndRadius - kStartRadius) * t;
        const int alpha = static_cast<int>(220.0 * (1.0 - t));

        // Right-click = blue; left-click = red. Helps reviewers spot right-clicks.
        const QColor stroke = r.rightClick
            ? QColor(80, 160, 255, alpha)
            : QColor(255, 70, 70, alpha);
        const QColor fill = r.rightClick
            ? QColor(80, 160, 255, alpha / 4)
            : QColor(255, 70, 70, alpha / 4);

        p.setPen(QPen(stroke, 2.5));
        p.setBrush(fill);
        p.drawEllipse(r.localPos, radius, radius);
    }
}

// ---------------------------------------------------------------------------
// ClickIndicatorOverlay — per-screen window manager
// ---------------------------------------------------------------------------

ClickIndicatorOverlay::ClickIndicatorOverlay(QObject *parent)
    : QObject(parent)
{
    for (QScreen *s : QGuiApplication::screens())
        onScreenAdded(s);

    // Track display hot-plug so a monitor connected mid-recording gets its
    // own overlay and a disconnected one doesn't leave a zombie window.
    connect(qGuiApp, &QGuiApplication::screenAdded,
            this, &ClickIndicatorOverlay::onScreenAdded);
    connect(qGuiApp, &QGuiApplication::screenRemoved,
            this, &ClickIndicatorOverlay::onScreenRemoved);
}

ClickIndicatorOverlay::~ClickIndicatorOverlay()
{
    // The per-screen windows are top-level (unparented) widgets — we own them.
    qDeleteAll(m_windows);
    m_windows.clear();
}

void ClickIndicatorOverlay::onScreenAdded(QScreen *screen)
{
    if (!screen || m_windows.contains(screen)) return;
    auto *w = new ClickRippleWindow(screen);
    m_windows.insert(screen, w);
    if (m_visible) w->showOverlay();
}

void ClickIndicatorOverlay::onScreenRemoved(QScreen *screen)
{
    if (ClickRippleWindow *w = m_windows.take(screen)) {
        w->hideOverlay();
        w->deleteLater();
    }
}

void ClickIndicatorOverlay::showOverlay()
{
    m_visible = true;
    for (ClickRippleWindow *w : std::as_const(m_windows))
        w->showOverlay();
}

void ClickIndicatorOverlay::hideOverlay()
{
    m_visible = false;
    for (ClickRippleWindow *w : std::as_const(m_windows))
        w->hideOverlay();
}

void ClickIndicatorOverlay::addRipple(QPoint globalPos, bool rightClick)
{
    if (!m_visible) return;
    // Route the ripple to the window owning the display under the click —
    // each window only spans its own screen, so global coords map cleanly.
    QScreen *s = QGuiApplication::screenAt(globalPos);
    ClickRippleWindow *w = s ? m_windows.value(s) : nullptr;
    if (!w) return;
    w->addRipple(globalPos, rightClick);
}
