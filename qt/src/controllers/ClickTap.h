#ifndef CLICKTAP_H
#define CLICKTAP_H

#include <QObject>

#include "SnapforgeClient.h"

// Permission probe for the global mouse-down tap. Wraps the snapforge_clicks_*
// use-case FFI (which owns the platform-specific event tap inside
// snapforge-app). start() installs the tap and returns whether it succeeded;
// the recording pipeline bakes click ripples in Rust, so nothing on the Qt
// side consumes the events — this exists purely to detect a missing grant and
// warn before recording.
//
// On macOS this requires the Input Monitoring TCC grant; start() returns
// false when the grant is missing or the tap can't be created.
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

private:
    // Required FFI callback; the events are unused (ripples are baked in Rust).
    static void onClickStatic(double x, double y, int rightClick,
                              void *userData);

    sf::ClickHandle m_handle;
};

#endif // CLICKTAP_H
