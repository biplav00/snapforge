#pragma once

#include <QObject>
#include <QSystemTrayIcon>
#include <QIcon>
#include <QString>

class QMenu;
class QTimer;

// Owns the QSystemTrayIcon, its menu (rebuilt on state change), the idle and
// recording-pill icon factories, and the pulse timer that animates the pill.
//
// Knows nothing about RecordingManager, overlays, or windows directly — main
// (and RecordingController) drive it through enterRecordingState/leaveRecordingState/
// setPaused/updateElapsed/showMessage, and react to the action* signals it
// emits when a menu item is triggered.
class TrayIcon : public QObject {
    Q_OBJECT
public:
    explicit TrayIcon(QObject *parent = nullptr);
    ~TrayIcon() override;

    // Initial menu (normal/idle layout) + show. Call once at startup.
    void initialize();

public slots:
    // Swap to the pulsing-pill icon, switch menu to recording layout, start
    // the pulse timer, update tooltip.
    void enterRecordingState(bool paused);

    // Swap back to the idle icon, stop the pulse, restore the normal menu.
    void leaveRecordingState();

    // Re-render the pill so its MM:SS text reflects the new elapsed count.
    // Safe to call only while in the recording state.
    void updateElapsed(int seconds);

    // Toggle the pause appearance (bars vs dot) and rebuild the recording
    // menu so Pause ↔ Resume swap.
    void setPaused(bool paused);

    // Pass-through to QSystemTrayIcon::showMessage so callers don't need to
    // reach the underlying object directly.
    void showMessage(const QString &title,
                     const QString &body,
                     QSystemTrayIcon::MessageIcon icon,
                     int msTimeout);

    // Rebuild the active menu so updated hotkey glyphs show after a rebind.
    void refreshMenu();

signals:
    // Idle-menu actions
    void actionScreenshot();
    void actionFullscreen();
    void actionRecordToggle();
    void actionHistory();
    void actionPreferences();
    void actionQuit();

    // Recording-menu actions
    void actionPauseRecording();
    void actionResumeRecording();
    void actionStopRecording();

private:
    QIcon makeIdleIcon() const;
    QIcon makeRecordingPillIcon(double alpha, bool paused, int seconds) const;
    QIcon makeStopIcon() const;

    void buildNormalMenu();
    void buildRecordingMenu();

    void refreshPill(double alpha);

    QSystemTrayIcon *m_tray = nullptr;
    // Dedicated menu-bar stop button shown only while recording: a single
    // click stops the capture without opening the main tray's context menu.
    // Has no context menu of its own, so activation fires actionStopRecording
    // directly.
    QSystemTrayIcon *m_stopTray = nullptr;
    QMenu           *m_menu = nullptr;
    QTimer          *m_pulseTimer = nullptr;
    QIcon            m_idleIcon;
    QIcon            m_stopIcon;

    bool m_inRecordingState = false;
    bool m_paused           = false;
    int  m_elapsedSeconds   = 0;
    bool m_pulseHigh        = true;
};
