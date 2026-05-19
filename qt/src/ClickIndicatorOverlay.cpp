#include "ClickIndicatorOverlay.h"

#include <QPainter>
#include <QPainterPath>
#include <QPaintEvent>
#include <QScreen>
#include <QGuiApplication>
#include <QTimer>
#include <QWindow>
#include <algorithm>

#ifdef Q_OS_MACOS
// Implemented in ClickIndicatorOverlayMac.mm — keeps AppKit out of this
// translation unit so it can stay a plain .cpp.
void clickoverlay_configure_macos_window(QWindow *w);
#endif

ClickIndicatorOverlay::ClickIndicatorOverlay(QWidget *parent)
    : QWidget(parent,
              Qt::FramelessWindowHint
              | Qt::WindowStaysOnTopHint
              | Qt::Tool
              | Qt::WindowDoesNotAcceptFocus
              | Qt::BypassWindowManagerHint)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_TransparentForMouseEvents);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setFocusPolicy(Qt::NoFocus);

    m_anim = new QTimer(this);
    m_anim->setInterval(16);  // ~60fps
    connect(m_anim, &QTimer::timeout, this, &ClickIndicatorOverlay::tick);

    m_clock.start();
}

ClickIndicatorOverlay::~ClickIndicatorOverlay() = default;

void ClickIndicatorOverlay::resizeToVirtualDesktop()
{
    QRect vrect;
    for (QScreen *s : QGuiApplication::screens())
        vrect = vrect.united(s->geometry());
    if (vrect.isEmpty()) return;
    setGeometry(vrect);
}

void ClickIndicatorOverlay::showOverlay()
{
    resizeToVirtualDesktop();
    show();

#ifdef Q_OS_MACOS
    if (QWindow *w = windowHandle())
        clickoverlay_configure_macos_window(w);
#endif

    raise();
}

void ClickIndicatorOverlay::hideOverlay()
{
    m_anim->stop();
    m_ripples.clear();
    hide();
}

void ClickIndicatorOverlay::addRipple(QPoint globalPos, bool rightClick)
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

void ClickIndicatorOverlay::tick()
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

void ClickIndicatorOverlay::paintEvent(QPaintEvent *)
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
