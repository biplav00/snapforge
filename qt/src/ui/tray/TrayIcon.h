#pragma once

#include <QObject>
#include <QSystemTrayIcon>
#include <QIcon>
#include <QString>

class QMenu;
class QTimer;

// Owns the two menu-bar items: the idle Snapforge icon (with the normal menu)
// and the dedicated stop button shown only while recording. During a
// recording the idle item is hidden and the stop button is the sole menu-bar
// presence; it pulses gently and a left-click stops the capture. Right-click
// opens the recording menu (pause/resume, history, preferences, quit) so those
// actions stay reachable without a second menu-bar slot.
//
// Knows nothing about RecordingManager, overlays, or windows directly — main
// (and RecordingController) drive it through enterRecordingState/leaveRecordingState/
// setPaused/enterFinalizingState/showMessage, and react to the action* signals
// it emits when a menu item is triggered.
class TrayIcon : public QObject {
    Q_OBJECT
public:
    explicit TrayIcon(QObject *parent = nullptr);
    ~TrayIcon() override;

    // Initial menu (normal/idle layout) + show. Call once at startup.
    void initialize();

public slots:
    // Hide the idle item, show the pulsing stop button, switch to the
    // recording menu, and start the pulse.
    void enterRecordingState(bool paused);

    // Stop the pulse, hide the stop button, restore the idle icon/menu.
    void leaveRecordingState();

    // Stop was requested and the file is being finalized: show the idle item
    // again with a "Finalizing…" menu, hide the stop button.
    void enterFinalizingState();

    // Toggle the pulse (paused = static stop button) and rebuild the
    // recording menu so Pause ↔ Resume swap.
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
    QIcon makeStopIcon(double pulse = 0.0) const;

    void buildNormalMenu();
    void buildRecordingMenu();
    // Rebuild the menu for the current state on the next event-loop turn.
    // Menu changes are always deferred: most state changes are triggered
    // from one of this menu's own QActions, and clear()ing the menu (which
    // deletes that action) inside its triggered() handler is a use-after-free
    // waiting to happen. Coalesces multiple requests into one rebuild.
    void scheduleMenuRebuild();
    void rebuildMenuNow();

    // Re-render the stop icon at the current pulse phase.
    void refreshStopIcon();

    QSystemTrayIcon *m_tray = nullptr;
    // Dedicated menu-bar stop button shown only while recording: a single
    // click stops the capture; right-click opens the recording menu.
    QSystemTrayIcon *m_stopTray = nullptr;
    QMenu           *m_menu = nullptr;
    QTimer          *m_pulseTimer = nullptr;
    QIcon            m_idleIcon;

    bool m_inRecordingState = false;
    bool m_paused           = false;
    bool m_finalizing       = false;
    bool m_rebuildPending   = false;
    int  m_pulsePhase       = 0;
};
