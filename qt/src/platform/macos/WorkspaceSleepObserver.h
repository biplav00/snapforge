#ifndef WORKSPACE_SLEEP_OBSERVER_H
#define WORKSPACE_SLEEP_OBSERVER_H

#include <QObject>

class RecordingManager;

// Fix #10: auto-pause an active recording when the Mac goes to sleep and
// resume it on wake. Uses NSWorkspace notifications so we see sleep/wake
// regardless of which app has focus.
//
// Also re-arms app-level state after sleep: macOS can drop Carbon global
// hotkeys across a wake (or after a user-session switch), leaving the app
// running but every shortcut dead. `systemDidWake` lets main.cpp re-register
// them and reset the overlay's stale capture/session state.
class WorkspaceSleepObserver : public QObject {
    Q_OBJECT
public:
    explicit WorkspaceSleepObserver(RecordingManager *rec, QObject *parent = nullptr);
    ~WorkspaceSleepObserver();

signals:
    // Emitted on NSWorkspaceDidWakeNotification and
    // NSWorkspaceSessionDidBecomeActiveNotification.
    void systemDidWake();

private:
    RecordingManager *m_recording = nullptr;
    void *m_sleepToken   = nullptr;  // id
    void *m_wakeToken    = nullptr;  // id
    void *m_sessionToken = nullptr;  // id
};

#endif // WORKSPACE_SLEEP_OBSERVER_H
