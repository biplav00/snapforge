#include "TrayIcon.h"
#include "Shortcuts.h"

#include <QApplication>
#include <QMenu>
#include <QTimer>
#include <QPixmap>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QFont>
#include <QFontMetrics>
#include <QPolygonF>
#include <QPointF>
#include <QRectF>
#include <QVariant>
#include <cmath>

TrayIcon::TrayIcon(QObject *parent)
    : QObject(parent),
      m_tray(new QSystemTrayIcon(this)),
      m_stopTray(new QSystemTrayIcon(this)),
      m_menu(new QMenu()),
      m_pulseTimer(new QTimer(this)) {
    m_pulseTimer->setInterval(500);
    QObject::connect(m_pulseTimer, &QTimer::timeout, this, [this]() {
        m_pulseHigh = !m_pulseHigh;
        refreshPill(m_pulseHigh ? 1.0 : 0.35);
    });

    // Stop button has no context menu, so any activation (left click on macOS)
    // stops the recording outright.
    QObject::connect(m_stopTray, &QSystemTrayIcon::activated, this,
                     [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger ||
            reason == QSystemTrayIcon::DoubleClick ||
            reason == QSystemTrayIcon::Context) {
            emit actionStopRecording();
        }
    });
}

TrayIcon::~TrayIcon() {
    // m_menu is heap-allocated without a parent (QSystemTrayIcon is a QObject
    // and can't take widget parent ownership), so delete it explicitly.
    delete m_menu;
}

void TrayIcon::initialize() {
    qDebug("System tray available: %d", QSystemTrayIcon::isSystemTrayAvailable());

    m_idleIcon = makeIdleIcon();
    m_tray->setIcon(m_idleIcon);
    m_tray->setToolTip("Snapforge");

    // Stop button starts hidden; only shown while recording (enterRecordingState).
    m_stopIcon = makeStopIcon();
    m_stopTray->setIcon(m_stopIcon);
    m_stopTray->setToolTip("Stop Recording");
    m_stopTray->hide();

    // Preserve the existing app-wide property used by miscellaneous tray
    // banners that don't have direct access to TrayIcon (e.g. save-failed
    // path inside saveImage).
    qApp->setProperty("systemTray", QVariant::fromValue(static_cast<QObject *>(m_tray)));

    buildNormalMenu();
    m_tray->setContextMenu(m_menu);
    m_tray->show();
    qDebug("Tray shown; visible=%d", m_tray->isVisible());
}

void TrayIcon::enterRecordingState(bool paused) {
    m_inRecordingState = true;
    m_paused = paused;
    buildRecordingMenu();
    m_tray->setToolTip(paused ? "Snapforge — Paused" : "Snapforge — Recording");
    m_stopTray->show();
    refreshPill(1.0);
    if (paused) {
        m_pulseTimer->stop();
    } else {
        m_pulseTimer->start();
    }
}

void TrayIcon::leaveRecordingState() {
    m_inRecordingState = false;
    m_paused = false;
    m_elapsedSeconds = 0;
    m_pulseTimer->stop();
    m_stopTray->hide();
    m_tray->setIcon(m_idleIcon);
    m_tray->setToolTip("Snapforge");
    buildNormalMenu();
}

void TrayIcon::updateElapsed(int seconds) {
    m_elapsedSeconds = seconds;
    if (m_inRecordingState) {
        refreshPill(1.0);
    }
}

void TrayIcon::setPaused(bool paused) {
    if (!m_inRecordingState) return;
    m_paused = paused;
    if (paused) {
        m_pulseTimer->stop();
        m_tray->setToolTip("Snapforge — Paused");
    } else {
        m_pulseTimer->start();
        m_tray->setToolTip("Snapforge — Recording");
    }
    refreshPill(1.0);
    buildRecordingMenu();
}

void TrayIcon::showMessage(const QString &title,
                           const QString &body,
                           QSystemTrayIcon::MessageIcon icon,
                           int msTimeout) {
    m_tray->showMessage(title, body, icon, msTimeout);
}

void TrayIcon::refreshPill(double alpha) {
    m_tray->setIcon(makeRecordingPillIcon(alpha, m_paused, m_elapsedSeconds));
}

// Idle tray icon — brand mark in white on a transparent background: an
// aperture-style six-blade hex ring with a small shutter notch in the top
// right. Distinct from generic crosshair / camera glyphs and reads as a
// capture/record motif at menu-bar size.
QIcon TrayIcon::makeIdleIcon() const {
    const qreal logicalSz = 18.0;
    // Match the pill/stop icons' 3.0 DPR so all three tray glyphs render at the
    // same crispness on a Retina menu bar.
    const qreal dpr = 3.0;
    const int sz = static_cast<int>(logicalSz * dpr);
    QPixmap pm(sz, sz);
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);

    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);

    const QColor ink(255, 255, 255, 255);
    QPen pen(ink, 1.2);
    pen.setCapStyle(Qt::RoundCap);
    pen.setJoinStyle(Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);

    // Outer rounded frame.
    const qreal frameMargin = 1.5;
    QRectF frame(frameMargin, frameMargin,
                 logicalSz - 2 * frameMargin, logicalSz - 2 * frameMargin);
    p.drawRoundedRect(frame, 3.5, 3.5);

    // Aperture: a hexagonal ring with three short blade lines that
    // converge toward the center, evoking a six-blade iris.
    const qreal cx = logicalSz / 2.0;
    const qreal cy = logicalSz / 2.0;
    const qreal r = 4.6;
    QPolygonF hex;
    for (int i = 0; i < 6; ++i) {
        // Tilt the hexagon 30° so a flat edge sits at the top — reads as
        // an aperture rather than a hex nut.
        qreal a = (M_PI / 3.0) * i + (M_PI / 6.0);
        hex << QPointF(cx + r * std::cos(a), cy + r * std::sin(a));
    }
    p.drawPolygon(hex);

    // Three blade ticks: from alternating hex vertices toward center,
    // stopping short to leave a clear pupil.
    const qreal pupil = 1.4;
    for (int i = 0; i < 6; i += 2) {
        QPointF v = hex[i];
        QPointF dir(cx - v.x(), cy - v.y());
        qreal len = std::hypot(dir.x(), dir.y());
        if (len <= pupil) continue;
        qreal t = (len - pupil) / len;
        QPointF endP(v.x() + dir.x() * t, v.y() + dir.y() * t);
        p.drawLine(v, endP);
    }

    // Shutter notch: small filled square in the top-right corner of the
    // frame, signaling "press to capture" — the unique flourish.
    p.setPen(Qt::NoPen);
    p.setBrush(ink);
    p.drawRoundedRect(QRectF(logicalSz - 4.6, frameMargin - 0.2, 2.4, 2.4),
                      0.6, 0.6);

    p.end();
    return QIcon(pm);
}

// Minimal recording indicator (mock D2): pulsing red dot + white "mm:ss"
// timer, no background pill, no REC label, no inline controls. All
// pause/stop actions live in the tray's context menu.
QIcon TrayIcon::makeRecordingPillIcon(double alpha, bool paused, int seconds) const {
    QString timeStr;
    if (seconds >= 3600) {
        timeStr = QStringLiteral("%1:%2:%3")
            .arg(seconds / 3600, 1, 10, QLatin1Char('0'))
            .arg((seconds / 60) % 60, 2, 10, QLatin1Char('0'))
            .arg(seconds % 60, 2, 10, QLatin1Char('0'));
    } else {
        timeStr = QStringLiteral("%1:%2")
            .arg(seconds / 60, 2, 10, QLatin1Char('0'))
            .arg(seconds % 60, 2, 10, QLatin1Char('0'));
    }

    QFont timeFont(QStringLiteral("Menlo"));
    timeFont.setPixelSize(15);
    timeFont.setWeight(QFont::Bold);
    QFontMetrics tfm(timeFont);
    const qreal timeW = tfm.horizontalAdvance(timeStr);

    const qreal padX = 4.0;
    const qreal dotR = 5.5;
    const qreal gap = 6.0;
    const int logicalH = 18;
    const int logicalW = static_cast<int>(std::ceil(
        padX + dotR * 2.0 + gap + timeW + padX));

    const qreal dpr = 3.0;
    QPixmap pm(static_cast<int>(logicalW * dpr),
               static_cast<int>(logicalH * dpr));
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);
    p.setRenderHint(QPainter::TextAntialiasing);

    const qreal cy = logicalH / 2.0;
    const qreal dotCx = padX + dotR;

    p.setPen(Qt::NoPen);
    if (paused) {
        // Two short bars in place of the dot — same footprint so the
        // overall layout doesn't shift when toggling pause.
        p.setBrush(QColor(255, 150, 150, 230));
        const qreal barW = 2.6;
        // Match the dot's diameter (2*dotR) so the glyph keeps the same
        // footprint and height when toggling pause.
        const qreal barH = dotR * 2.0;
        p.drawRoundedRect(QRectF(dotCx - dotR, cy - barH / 2.0, barW, barH),
                          0.8, 0.8);
        p.drawRoundedRect(QRectF(dotCx + dotR - barW, cy - barH / 2.0,
                                 barW, barH), 0.8, 0.8);
    } else {
        int halo = qBound(0, static_cast<int>(90 * alpha), 90);
        p.setBrush(QColor(255, 70, 70, halo));
        p.drawEllipse(QPointF(dotCx, cy), dotR + 2.5, dotR + 2.5);
        int dotAlpha = qBound(200, static_cast<int>(255 * alpha), 255);
        p.setBrush(QColor(255, 70, 70, dotAlpha));
        p.drawEllipse(QPointF(dotCx, cy), dotR, dotR);
    }

    p.setFont(timeFont);
    p.setPen(paused ? QColor(220, 220, 220, 240)
                    : QColor(255, 255, 255, 255));
    const qreal textX = padX + dotR * 2.0 + gap;
    p.drawText(QRectF(textX, 0, timeW + 2, logicalH),
               Qt::AlignVCenter | Qt::AlignLeft, timeStr);

    p.end();
    // Multi-color (red dot + white text), so render as a regular icon —
    // template mode would collapse the dot's red into the menu-bar tint.
    QIcon icon(pm);
    icon.setIsMask(false);
    return icon;
}

// Menu-bar stop button: a solid red rounded square — the universal stop glyph,
// tinted to match the recording dot so the pair reads as "recording / stop".
// Multi-color against the bar tint, so rendered as a regular (non-mask) icon.
QIcon TrayIcon::makeStopIcon() const {
    const qreal logicalSz = 18.0;
    const qreal dpr = 3.0;
    const int sz = static_cast<int>(logicalSz * dpr);
    QPixmap pm(sz, sz);
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);

    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(Qt::NoPen);
    p.setBrush(QColor(255, 70, 70, 255));

    const qreal side = 12.0;
    const qreal off = (logicalSz - side) / 2.0;
    p.drawRoundedRect(QRectF(off, off, side, side), 2.6, 2.6);

    p.end();
    QIcon icon(pm);
    icon.setIsMask(false);
    return icon;
}

// Build "Name\t⌘⇧S": the part after the tab lands in the menu's right-aligned
// shortcut column on macOS. Glyphs come from the shared config, so a rebind in
// Preferences shows here after refreshMenu() — no hardcoded chord strings.
// The caller-supplied Snapshot means one config load per menu build, not one
// per label.
static QString trayLabel(const shortcuts::Snapshot &cfg,
                         const QString &name, const char *actionKey) {
    const QString g = shortcuts::glyphs(cfg.chord(QString::fromLatin1(actionKey)));
    return g.isEmpty() ? name : name + QLatin1Char('\t') + g;
}

void TrayIcon::buildNormalMenu() {
    m_menu->clear();
    const shortcuts::Snapshot cfg;

    m_menu->addAction(trayLabel(cfg, QStringLiteral("Screenshot"), "screenshot"), this, &TrayIcon::actionScreenshot);
    m_menu->addAction(trayLabel(cfg, QStringLiteral("Capture Fullscreen"), "fullscreen"), this, &TrayIcon::actionFullscreen);
    m_menu->addAction(trayLabel(cfg, QStringLiteral("Record"), "record"), this, &TrayIcon::actionRecordToggle);

    m_menu->addSeparator();

    m_menu->addAction(trayLabel(cfg, QStringLiteral("History"), "history"), this, &TrayIcon::actionHistory);
    m_menu->addAction(trayLabel(cfg, QStringLiteral("Preferences"), "preferences"), this, &TrayIcon::actionPreferences);

    m_menu->addSeparator();

    m_menu->addAction(QStringLiteral("Quit"), this, &TrayIcon::actionQuit);
}

void TrayIcon::buildRecordingMenu() {
    m_menu->clear();
    const shortcuts::Snapshot cfg;

    QAction *recordingLabel = m_menu->addAction(
        m_paused ? QStringLiteral("⏸ Paused") : QStringLiteral("● Recording..."));
    recordingLabel->setEnabled(false);

    if (m_paused) {
        m_menu->addAction(QStringLiteral("▶ Resume Recording"), this, &TrayIcon::actionResumeRecording);
    } else {
        m_menu->addAction(QStringLiteral("⏸ Pause Recording"), this, &TrayIcon::actionPauseRecording);
    }

    m_menu->addAction(trayLabel(cfg, QStringLiteral("■ Stop Recording"), "record"), this, &TrayIcon::actionStopRecording);

    m_menu->addSeparator();

    m_menu->addAction(trayLabel(cfg, QStringLiteral("History"), "history"), this, &TrayIcon::actionHistory);
    m_menu->addAction(trayLabel(cfg, QStringLiteral("Preferences"), "preferences"), this, &TrayIcon::actionPreferences);

    m_menu->addSeparator();

    m_menu->addAction(QStringLiteral("Quit"), this, &TrayIcon::actionQuit);
}

void TrayIcon::refreshMenu() {
    // Rebuild the menu currently shown (normal vs recording) so updated chord
    // glyphs take effect after a Preferences rebind.
    if (m_inRecordingState)
        buildRecordingMenu();
    else
        buildNormalMenu();
}
