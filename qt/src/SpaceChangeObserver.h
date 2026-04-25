#ifndef SPACECHANGEOBSERVER_H
#define SPACECHANGEOBSERVER_H

#include <QObject>

class QWidget;

// Re-activates a target widget whenever the user switches Spaces (e.g.
// returns from a fullscreen app). Without this, the overlay window loses
// key-window status on Space change and keyboard shortcuts go dead.
class SpaceChangeObserver : public QObject {
    Q_OBJECT
public:
    explicit SpaceChangeObserver(QWidget *target, QObject *parent = nullptr);
    ~SpaceChangeObserver();

private:
    QWidget *m_target = nullptr;
    void *m_token = nullptr; // NSNotificationCenter observer token
};

#endif // SPACECHANGEOBSERVER_H
