#ifndef CLICKTAP_H
#define CLICKTAP_H

#include <QObject>
#include <QPoint>

// Platform-agnostic global mouse-down listener. Wraps the
// snapforge_clicks_* use-case FFI (which owns the platform-specific event
// tap inside snapforge-app). Emits clicked() on the Qt main thread for
// every left/right mouse-down system-wide while running.
//
// On macOS this requires the Input Monitoring TCC grant; start() returns
// false when the grant is missing or the tap can't be created.
//
// Replaces the macOS-only ClickEventTap from Phase 2C — the platform code
// now lives in Rust, so this class no longer needs an Objective-C++
// translation unit.
class ClickTap : public QObject {
    Q_OBJECT
public:
    explicit ClickTap(QObject *parent = nullptr);
    ~ClickTap() override;

    // Returns true if the tap was installed and is streaming events.
    // Returns false if the FFI rejected the call (typically a missing
    // permission grant — read snapforge_app_last_error for details).
    bool start();
    void stop();
    bool isRunning() const;

signals:
    void clicked(QPoint globalPos, bool rightClick);

private:
    // Fires on a Rust-owned thread (NOT the Qt main thread). Re-dispatches
    // to the Qt thread via QMetaObject::invokeMethod so signal subscribers
    // can safely touch GUI state.
    static void onClickStatic(double x, double y, int rightClick,
                              void *userData);

    void *m_handle = nullptr;
};

#endif // CLICKTAP_H
