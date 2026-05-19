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
                    // pauseRecording is idempotent — it checks state itself,
                    // so we don't need to pre-check isRecording/isPaused.
                    QMetaObject::invokeMethod(m, "pauseRecording",
                                              Qt::QueuedConnection);
                }];
    id wakeToken = [center
        addObserverForName:NSWorkspaceDidWakeNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                    RecordingManager *m = guard.data();
                    if (!m) return;
                    // resumeRecording is idempotent — slot guards state.
                    QMetaObject::invokeMethod(m, "resumeRecording",
                                              Qt::QueuedConnection);
                }];

    // Retain the observer tokens so we can remove them later.
    m_sleepToken = (__bridge_retained void *)sleepToken;
    m_wakeToken  = (__bridge_retained void *)wakeToken;
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
}
