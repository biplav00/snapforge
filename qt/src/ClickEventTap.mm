#include "ClickEventTap.h"

#include <QPoint>
#include <QPointer>
#include <QMetaObject>
#include <Qt>

#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

class ClickEventTapPrivate {
public:
    CFMachPortRef tap = nullptr;
    CFRunLoopSourceRef src = nullptr;
    bool running = false;
};

namespace {

CGEventRef eventCallback(CGEventTapProxy /*proxy*/,
                         CGEventType type,
                         CGEventRef event,
                         void *userInfo)
{
    // If the tap is disabled by timeout (slow handler) or by user
    // intervention, re-enable it so we don't go silent.
    if (type == kCGEventTapDisabledByTimeout
        || type == kCGEventTapDisabledByUserInput) {
        auto *self = static_cast<ClickEventTap *>(userInfo);
        // Re-enable on next runloop tick; can't deref private here without
        // exposing more, so emit a sentinel via the public API.
        QMetaObject::invokeMethod(self, [self]() { self->start(); },
                                  Qt::QueuedConnection);
        return event;
    }

    if (type != kCGEventLeftMouseDown && type != kCGEventRightMouseDown)
        return event;

    CGPoint loc = CGEventGetLocation(event);
    bool rightClick = (type == kCGEventRightMouseDown);
    // CGEvent location is in flipped Cocoa global coords; QPoint uses Qt
    // screen coords which match (top-left origin) on macOS for Qt 6.
    QPoint p(static_cast<int>(loc.x), static_cast<int>(loc.y));

    auto *self = static_cast<ClickEventTap *>(userInfo);
    QPointer<ClickEventTap> guard(self);
    dispatch_async(dispatch_get_main_queue(), ^{
        if (guard) emit guard->clicked(p, rightClick);
    });

    // Always pass the event through unchanged.
    return event;
}

} // namespace

ClickEventTap::ClickEventTap(QObject *parent)
    : QObject(parent), d(new ClickEventTapPrivate)
{}

ClickEventTap::~ClickEventTap()
{
    stop();
    delete d;
}

bool ClickEventTap::isRunning() const { return d->running; }

bool ClickEventTap::start()
{
    if (d->running) return true;

    const CGEventMask mask =
        CGEventMaskBit(kCGEventLeftMouseDown)
        | CGEventMaskBit(kCGEventRightMouseDown);

    // kCGSessionEventTap = session-wide; kCGHeadInsertEventTap = before others.
    // kCGEventTapOptionListenOnly = passive listener, no event modification.
    d->tap = CGEventTapCreate(kCGSessionEventTap,
                              kCGHeadInsertEventTap,
                              kCGEventTapOptionListenOnly,
                              mask,
                              eventCallback,
                              this);
    if (!d->tap) {
        // Most common cause: Input Monitoring permission not granted.
        return false;
    }

    d->src = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, d->tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), d->src, kCFRunLoopCommonModes);
    CGEventTapEnable(d->tap, true);
    d->running = true;
    return true;
}

void ClickEventTap::stop()
{
    if (!d->running && !d->tap) return;

    if (d->tap) CGEventTapEnable(d->tap, false);

    if (d->src) {
        CFRunLoopRemoveSource(CFRunLoopGetMain(), d->src, kCFRunLoopCommonModes);
        CFRelease(d->src);
        d->src = nullptr;
    }
    if (d->tap) {
        CFRelease(d->tap);
        d->tap = nullptr;
    }
    d->running = false;
}
