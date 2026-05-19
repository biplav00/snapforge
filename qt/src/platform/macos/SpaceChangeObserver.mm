#include "SpaceChangeObserver.h"

#include <QPointer>
#include <QWidget>
#include <objc/message.h>
#include <objc/runtime.h>

#import <AppKit/AppKit.h>

SpaceChangeObserver::SpaceChangeObserver(QWidget *target, QObject *parent)
    : QObject(parent), m_target(target)
{
    NSNotificationCenter *center = [[NSWorkspace sharedWorkspace] notificationCenter];
    QPointer<QWidget> guard(m_target);

    id token = [center
        addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                    QWidget *w = guard.data();
                    if (!w || !w->isVisible()) return;
                    QMetaObject::invokeMethod(w, [w]() {
                        w->raise();
                        w->activateWindow();
                        id nsView = (__bridge id)reinterpret_cast<void *>(w->winId());
                        id nsWindow = ((id (*)(id, SEL))objc_msgSend)(
                            nsView, sel_registerName("window"));
                        if (nsWindow) {
                            ((void (*)(id, SEL))objc_msgSend)(
                                nsWindow, sel_registerName("makeKeyWindow"));
                        }
                    }, Qt::QueuedConnection);
                }];

    m_token = (__bridge_retained void *)token;
}

SpaceChangeObserver::~SpaceChangeObserver()
{
    if (m_token) {
        id token = (__bridge_transfer id)m_token;
        [[[NSWorkspace sharedWorkspace] notificationCenter] removeObserver:token];
        m_token = nullptr;
    }
}
