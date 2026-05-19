#ifndef CLICKEVENTTAP_H
#define CLICKEVENTTAP_H

#include <QObject>
#include <QPoint>

class ClickEventTapPrivate;

// Global mouse-down listener (macOS CGEventTap). Emits clicked() on the
// main thread for every left/right mouse-down system-wide while running.
// Requires the Input Monitoring TCC grant.
class ClickEventTap : public QObject {
    Q_OBJECT
public:
    explicit ClickEventTap(QObject *parent = nullptr);
    ~ClickEventTap() override;

    // Returns true if the tap was installed and enabled.
    // Returns false if Input Monitoring permission is missing or the
    // tap could not be created.
    bool start();
    void stop();
    bool isRunning() const;

signals:
    void clicked(QPoint globalPos, bool rightClick);

private:
    ClickEventTapPrivate *d = nullptr;
};

#endif // CLICKEVENTTAP_H
