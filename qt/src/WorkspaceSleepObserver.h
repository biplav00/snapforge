#ifndef WORKSPACE_SLEEP_OBSERVER_H
#define WORKSPACE_SLEEP_OBSERVER_H

#include <QObject>

class RecordingManager;

// Fix #10: auto-pause an active recording when the Mac goes to sleep and
// resume it on wake. Uses NSWorkspace notifications so we see sleep/wake
// regardless of which app has focus.
class WorkspaceSleepObserver : public QObject {
    Q_OBJECT
public:
    explicit WorkspaceSleepObserver(RecordingManager *rec, QObject *parent = nullptr);
    ~WorkspaceSleepObserver();

private:
    RecordingManager *m_recording = nullptr;
    void *m_sleepToken = nullptr;  // id
    void *m_wakeToken  = nullptr;  // id
};

#endif // WORKSPACE_SLEEP_OBSERVER_H
