#include "WorkspaceSleepObserver.h"
#include "RecordingManager.h"

#include <QPointer>

#import <AppKit/AppKit.h>

WorkspaceSleepObserver::WorkspaceSleepObserver(RecordingManager *rec, QObject *parent)
    : QObject(parent), m_recording(rec)
{
    NSNotificationCenter *center = [[NSWorkspace sharedWorkspace] notificationCenter];
    // Use QPointer so we don't chase a dangling RecordingManager* if the
    // manager is destroyed before the observer. QPointer is a Qt-native weak
    // ref for QObject subclasses — cheaper and clearer than __weak here.
    QPointer<RecordingManager> guard(m_recording);

    id sleepToken = [center
        addObserverForName:NSWorkspaceWillSleepNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                    RecordingManager *m = guard.data();
                    if (!m) return;
                    // handleSystemSleep pauses only an actively-running
                    // recording and remembers that the auto-pause was ours,
                    // so wake won't resume a manual pause.
                    // Called directly: this block already runs on the main
                    // queue (= the Qt GUI thread). Re-queuing it through the
                    // Qt event loop let the system sleep before the pause
                    // actually happened.
                    m->handleSystemSleep();
                }];
    // QPointer-to-self guards the notification blocks: a block queued just
    // before destruction must not emit through a dangling observer.
    QPointer<WorkspaceSleepObserver> self(this);
    id wakeToken = [center
        addObserverForName:NSWorkspaceDidWakeNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                    RecordingManager *m = guard.data();
                    if (!m) return;
                    // handleSystemWake resumes only if the sleep handler's
                    // auto-pause is what paused the recording.
                    m->handleSystemWake();
                    // Let main.cpp re-arm the global hotkeys and clear any
                    // overlay session that was live when the lid closed.
                    if (self) emit self->systemDidWake();
                }];

    // Fast user switching / logout+login tears down the process's Carbon
    // hotkey bindings on some macOS versions. Treat session activation like a
    // wake for re-arming purposes (but don't touch recording: it was not
    // necessarily paused by sleep).
    id sessionToken = [center
        addObserverForName:NSWorkspaceSessionDidBecomeActiveNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                    if (self) emit self->systemDidWake();
                }];

    // Retain the observer tokens so we can remove them later.
    m_sleepToken   = (__bridge_retained void *)sleepToken;
    m_wakeToken    = (__bridge_retained void *)wakeToken;
    m_sessionToken = (__bridge_retained void *)sessionToken;
}

WorkspaceSleepObserver::~WorkspaceSleepObserver()
{
    NSNotificationCenter *center = [[NSWorkspace sharedWorkspace] notificationCenter];
    if (m_sleepToken) {
        id token = (__bridge_transfer id)m_sleepToken;
        [center removeObserver:token];
        m_sleepToken = nullptr;
    }
    if (m_wakeToken) {
        id token = (__bridge_transfer id)m_wakeToken;
        [center removeObserver:token];
        m_wakeToken = nullptr;
    }
    if (m_sessionToken) {
        id token = (__bridge_transfer id)m_sessionToken;
        [center removeObserver:token];
        m_sessionToken = nullptr;
    }
}
