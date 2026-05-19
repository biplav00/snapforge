#include "ClickTap.h"

#include <QMetaObject>
#include <QPointer>
#include <Qt>

#include "snapforge_ffi.h"

ClickTap::ClickTap(QObject *parent)
    : QObject(parent) {}

ClickTap::~ClickTap() {
    stop();
}

bool ClickTap::isRunning() const {
    return m_handle != nullptr;
}

bool ClickTap::start() {
    if (m_handle) return true;
    m_handle = snapforge_clicks_start(&ClickTap::onClickStatic, this);
    return m_handle != nullptr;
}

void ClickTap::stop() {
    if (!m_handle) return;
    snapforge_clicks_stop(m_handle);
    snapforge_clicks_free_handle(m_handle);
    m_handle = nullptr;
}

void ClickTap::onClickStatic(double x, double y, int rightClick,
                             void *userData) {
    auto *self = static_cast<ClickTap *>(userData);
    QPoint p(static_cast<int>(x), static_cast<int>(y));
    bool right = rightClick != 0;
    // QPointer survives the hop to the Qt main thread — if `self` is
    // destroyed between the FFI thread firing this callback and the queued
    // slot running, we drop the click silently instead of dereferencing a
    // dangling pointer.
    QPointer<ClickTap> guard(self);
    QMetaObject::invokeMethod(
        self,
        [guard, p, right]() {
            if (guard) emit guard->clicked(p, right);
        },
        Qt::QueuedConnection);
}
