#include "TrayIcon.h"
#include "Shortcuts.h"

#include <QApplication>
#include <QCursor>
#include <QMenu>
#include <QTimer>
#include <QPixmap>
#include <QPainter>
#include <QPen>
#include <QPolygonF>
#include <QPointF>
#include <QRectF>
#include <QVariant>
#include <cmath>

namespace {
// Qt's Cocoa tray implementation selects a pixmap by its PHYSICAL height and
// caps it at (menu-bar thickness - 4) * qApp->devicePixelRatio(). Rendering at
// a fixed 3x on a 2x display made the icon taller than that cap, so AppKit
// scaled the whole pill down (shrinking the timer text by a third). Match the
// same DPR Qt uses so the pixmap is used at its intended size.
qreal trayDevicePixelRatio() {
    const qreal dpr = qApp ? qApp->devicePixelRatio() : 1.0;
    return qMax(1.0, dpr);
}

// Six steps over 1.2s: 0 → 0.35 → 0.7 → 1 → 0.7 → 0.35. The tray icon is a
// static pixmap, so the halo is redrawn at each step; 200ms is frequent enough
// to read as a smooth breath without churning the status item.
constexpr double kPulseSteps[] = {0.0, 0.35, 0.70, 1.0, 0.70, 0.35};
constexpr int kPulseStepCount = sizeof(kPulseSteps) / sizeof(kPulseSteps[0]);
} // namespace

TrayIcon::TrayIcon(QObject *parent)
    : QObject(parent),
      m_tray(new QSystemTrayIcon(this)),
      m_stopTray(new QSystemTrayIcon(this)),
      m_menu(new QMenu()),
      m_pulseTimer(new QTimer(this)) {
    m_pulseTimer->setInterval(200);
    QObject::connect(m_pulseTimer, &QTimer::timeout, this, [this]() {
        m_pulsePhase = (m_pulsePhase + 1) % kPulseStepCount;
        refreshStopIcon();
    });

    // The stop item has no context menu, so left-click is an instant stop.
    // Right-click opens the recording menu manually — that is the only way to
    // reach pause/resume and Quit while the idle item is hidden.
    QObject::connect(m_stopTray, &QSystemTrayIcon::activated, this,
                     [this](QSystemTrayIcon::ActivationReason reason) {
        if (m_finalizing) return;
        if (reason == QSystemTrayIcon::Context) {
            rebuildMenuNow();
            m_menu->popup(QCursor::pos());
            return;
        }
        if (reason == QSystemTrayIcon::Trigger ||
            reason == QSystemTrayIcon::DoubleClick) {
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
    m_stopTray->setIcon(makeStopIcon());
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
    m_finalizing = false;
    m_paused = paused;

    // The stop button is the only menu-bar item while recording.
    m_stopTray->show();
    m_tray->hide();

    scheduleMenuRebuild();
    m_stopTray->setToolTip("Stop Recording");
    m_pulsePhase = 0;
    refreshStopIcon();
    if (paused) {
        m_pulseTimer->stop();
    } else {
        m_pulseTimer->start();
    }
}

void TrayIcon::leaveRecordingState() {
    m_inRecordingState = false;
    m_finalizing = false;
    m_paused = false;
    m_pulseTimer->stop();
    m_pulsePhase = 0;
    m_stopTray->hide();
    m_tray->setIcon(m_idleIcon);
    m_tray->setToolTip("Snapforge");
    m_tray->show();
    scheduleMenuRebuild();
}

void TrayIcon::enterFinalizingState() {
    if (!m_inRecordingState) return;
    m_finalizing = true;
    m_pulseTimer->stop();
    m_pulsePhase = 0;
    m_stopTray->hide();
    m_tray->setIcon(m_idleIcon);
    m_tray->setToolTip("Snapforge — Finalizing recording…");
    m_tray->show();
    scheduleMenuRebuild();
}

void TrayIcon::scheduleMenuRebuild() {
    if (m_rebuildPending) return;
    m_rebuildPending = true;
    QTimer::singleShot(0, this, &TrayIcon::rebuildMenuNow);
}

void TrayIcon::rebuildMenuNow() {
    m_rebuildPending = false;
    if (m_inRecordingState)
        buildRecordingMenu();
    else
        buildNormalMenu();
}

void TrayIcon::setPaused(bool paused) {
    if (!m_inRecordingState || m_finalizing) return;
    m_paused = paused;
    m_pulsePhase = 0;
    if (paused) {
        m_pulseTimer->stop();
    } else {
        m_pulseTimer->start();
    }
    refreshStopIcon();
    scheduleMenuRebuild();
}

void TrayIcon::showMessage(const QString &title,
                           const QString &body,
                           QSystemTrayIcon::MessageIcon icon,
                           int msTimeout) {
    // While recording the idle item is hidden; post through whichever item is
    // actually in the menu bar so the banner still has a live host.
    QSystemTrayIcon *target = m_stopTray->isVisible() ? m_stopTray : m_tray;
    target->showMessage(title, body, icon, msTimeout);
}

void TrayIcon::refreshStopIcon() {
    m_stopTray->setIcon(makeStopIcon(m_paused ? 0.0 : kPulseSteps[m_pulsePhase]));
}

// Idle tray icon — brand mark in white on a transparent background: an
// aperture-style six-blade hex ring with a small shutter notch in the top
// right. Distinct from generic crosshair / camera glyphs and reads as a
// capture/record motif at menu-bar size.
QIcon TrayIcon::makeIdleIcon() const {
    const qreal logicalSz = 18.0;
    // Same DPR as the pill/stop icons so all three tray glyphs stay crisp and
    // avoid the Cocoa tray's downscaling path.
    const qreal dpr = trayDevicePixelRatio();
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

// Menu-bar stop button: a solid red rounded square with an optional soft halo
// used for the recording pulse. Multi-color against the bar tint, so rendered
// as a regular (non-mask) icon.
QIcon TrayIcon::makeStopIcon(double pulse) const {
    const qreal logicalSz = 20.0;
    const qreal dpr = trayDevicePixelRatio();
    const int sz = static_cast<int>(logicalSz * dpr);
    QPixmap pm(sz, sz);
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);

    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(Qt::NoPen);

    const qreal side = 13.0;
    const qreal off = (logicalSz - side) / 2.0;

    // Soft halo behind the square. It stays within the 20px pixmap: the
    // square occupies 3.5..16.5, and the halo extends 2.5px further.
    if (pulse > 0.0) {
        const qreal haloInset = 2.5;
        p.setBrush(QColor(255, 70, 70, static_cast<int>(90.0 * pulse)));
        p.drawRoundedRect(QRectF(off - haloInset, off - haloInset,
                                 side + haloInset * 2.0,
                                 side + haloInset * 2.0),
                          4.5, 4.5);
    }

    p.setBrush(QColor(255, 70, 70, 255));
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
        m_finalizing ? QStringLiteral("… Finalizing recording")
        : m_paused   ? QStringLiteral("⏸ Paused")
                     : QStringLiteral("● Recording..."));
    recordingLabel->setEnabled(false);

    QAction *pauseResume = m_paused
        ? m_menu->addAction(QStringLiteral("▶ Resume Recording"), this, &TrayIcon::actionResumeRecording)
        : m_menu->addAction(QStringLiteral("⏸ Pause Recording"), this, &TrayIcon::actionPauseRecording);
    QAction *stop = m_menu->addAction(trayLabel(cfg, QStringLiteral("■ Stop Recording"), "record"),
                                      this, &TrayIcon::actionStopRecording);
    // Nothing to pause or stop while the file is being finalized.
    pauseResume->setEnabled(!m_finalizing);
    stop->setEnabled(!m_finalizing);

    m_menu->addSeparator();

    m_menu->addAction(trayLabel(cfg, QStringLiteral("History"), "history"), this, &TrayIcon::actionHistory);
    m_menu->addAction(trayLabel(cfg, QStringLiteral("Preferences"), "preferences"), this, &TrayIcon::actionPreferences);

    m_menu->addSeparator();

    m_menu->addAction(QStringLiteral("Quit"), this, &TrayIcon::actionQuit);
}

void TrayIcon::refreshMenu() {
    // Rebuild the menu currently shown (normal vs recording) so updated chord
    // glyphs take effect after a Preferences rebind.
    scheduleMenuRebuild();
}
