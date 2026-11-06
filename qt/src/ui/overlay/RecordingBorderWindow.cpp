#include "RecordingBorderWindow.h"

#include <QGuiApplication>
#include <QPainter>
#include <QPen>

#ifdef Q_OS_MACOS
#include <objc/message.h>
#include <objc/runtime.h>
#endif

namespace {
// The window is expanded by this many logical pixels on each side so the
// hairline and its faint glow sit just outside the captured region instead of
// covering its edge pixels. Keep in sync with paintEvent's insets.
constexpr int kBorderInset = 8;
// 1px pen centred half a pixel inside the region boundary: it spans the last
// pixel before the captured area, so no recorded pixel is touched.
constexpr qreal kLineInset = kBorderInset - 0.5;
constexpr qreal kCornerRadius = 4.0;
} // namespace

RecordingBorderWindow::RecordingBorderWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint |
                         Qt::Tool | Qt::WindowTransparentForInput |
                         Qt::WindowDoesNotAcceptFocus) {
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_TransparentForMouseEvents);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setFocusPolicy(Qt::NoFocus);
}

void RecordingBorderWindow::setRegion(const QRect &globalRect) {
    const QRect normalized = globalRect.normalized();
    if (normalized.width() <= 0 || normalized.height() <= 0) {
        clear();
        return;
    }

    m_region = normalized;
    // Expand outward: the captured region begins kBorderInset pixels inside
    // this widget, and paintEvent draws the line against that boundary.
    setGeometry(m_region.adjusted(-kBorderInset, -kBorderInset,
                                  kBorderInset, kBorderInset));
}

void RecordingBorderWindow::showForPreparedRegion() {
    if (m_region.isEmpty())
        return;

    if (!m_nativeConfigured) {
        // Force native window creation before the first frame so the
        // sharingType/level settings are in place before it can be captured.
        (void)winId();
        applyNativeWindowStyle();
        m_nativeConfigured = true;
    }

    setGeometry(m_region.adjusted(-kBorderInset, -kBorderInset,
                                  kBorderInset, kBorderInset));
    show();
    raise();
    update();
}

void RecordingBorderWindow::setPaused(bool paused) {
    if (m_paused == paused)
        return;
    m_paused = paused;
    update();
}

void RecordingBorderWindow::clear() {
    m_region = QRect();
    m_paused = false;
    hide();
}

void RecordingBorderWindow::paintEvent(QPaintEvent * /*event*/) {
    if (m_region.isEmpty())
        return;

    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(Qt::NoBrush);

    // Faint glow: a few wide strokes layered outward from the hairline. They
    // stay between the widget edge and the captured region, so the effect is
    // only ever seen on screen — never baked into the recording.
    if (!m_paused) {
        struct GlowPass {
            qreal inset;
            qreal width;
            int alpha;
        };
        static constexpr GlowPass kGlow[] = {
            {6.5, 3.0, 18},
            {5.5, 5.0, 11},
            {4.5, 7.0, 7},
            {3.5, 9.0, 4},
        };
        for (const auto &pass : kGlow) {
            QPen glow(QColor(255, 70, 70, pass.alpha), pass.width);
            glow.setJoinStyle(Qt::RoundJoin);
            p.setPen(glow);
            const qreal radius = kCornerRadius + (kLineInset - pass.inset);
            p.drawRoundedRect(QRectF(rect()).adjusted(pass.inset, pass.inset,
                                                      -pass.inset, -pass.inset),
                              radius, radius);
        }
    }

    // 1px translucent hairline with soft corners. Paused switches to an even
    // fainter dashed line and drops the glow.
    QPen pen(m_paused ? QColor(255, 170, 170, 115)
                      : QColor(255, 70, 70, 128),
             1.0);
    pen.setJoinStyle(Qt::RoundJoin);
    pen.setCapStyle(Qt::FlatCap);
    if (m_paused) {
        pen.setStyle(Qt::CustomDashLine);
        pen.setDashPattern({4.0, 3.0});
    }
    p.setPen(pen);
    p.drawRoundedRect(QRectF(rect()).adjusted(kLineInset, kLineInset,
                                              -kLineInset, -kLineInset),
                      kCornerRadius, kCornerRadius);
}

void RecordingBorderWindow::applyNativeWindowStyle() {
#ifdef Q_OS_MACOS
    if (!QGuiApplication::platformName().startsWith(QLatin1String("cocoa")))
        return;

    auto *nsView = reinterpret_cast<id>(winId());
    id nsWindow = ((id (*)(id, SEL))objc_msgSend)(
        nsView, sel_registerName("window"));
    if (!nsWindow)
        return;

    // Float above normal windows (same level as the capture overlay), follow
    // the user across Spaces/fullscreen apps, and never steal focus or clicks.
    ((void (*)(id, SEL, long))objc_msgSend)(
        nsWindow, sel_registerName("setLevel:"), 1000L);
    const unsigned long behavior = (1UL << 0) | (1UL << 4) | (1UL << 6) | (1UL << 8);
    ((void (*)(id, SEL, unsigned long))objc_msgSend)(
        nsWindow, sel_registerName("setCollectionBehavior:"), behavior);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setIgnoresMouseEvents:"), YES);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setHidesOnDeactivate:"), NO);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setHasShadow:"), NO);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setOpaque:"), NO);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setMovable:"), NO);
    ((void (*)(id, SEL, BOOL))objc_msgSend)(
        nsWindow, sel_registerName("setMovableByWindowBackground:"), NO);
    ((void (*)(id, SEL, unsigned long))objc_msgSend)(
        nsWindow, sel_registerName("setStyleMask:"), 0UL); // borderless

    // NSWindowSharingNone: ScreenCaptureKit and CGWindowList captures skip
    // this window, so the indicator never appears in the recording.
    ((void (*)(id, SEL, unsigned long))objc_msgSend)(
        nsWindow, sel_registerName("setSharingType:"), 0UL);
#endif
}
