#pragma once

#include <QObject>
#include <QString>

class RecordingManager;
class TrayIcon;
class ClickIndicatorOverlay;
class ClickTap;
class PreferencesWindow;

// Wires RecordingManager's lifecycle signals (started/stopped/paused/resumed/
// error/elapsedChanged) to the user-visible feedback they trigger:
//   * tray pill state + tooltip + menu swap (TrayIcon)
//   * click visualizer overlay + global click tap (ClickTap, all platforms)
//   * clipboard-copy-on-stop of the finished file URL
//   * history.add (currently implicit via the recording pipeline)
//   * deferred QMessageBox modal on error
//
// Construct after all collaborators exist; the controller installs its
// connections in its constructor. Lifetime is bound to its QObject parent.
class RecordingController : public QObject {
    Q_OBJECT
public:
    RecordingController(RecordingManager      *recording,
                        TrayIcon              *tray,
                        ClickIndicatorOverlay *clickOverlay,
                        ClickTap              *clickTap,
                        PreferencesWindow     *prefs,
                        QObject               *parent = nullptr);

private:
    RecordingManager      *m_recording    = nullptr;
    TrayIcon              *m_tray         = nullptr;
    ClickIndicatorOverlay *m_clickOverlay = nullptr;
    ClickTap              *m_clickTap     = nullptr;
    PreferencesWindow     *m_prefs        = nullptr;

    void onStarted(const QString &path);
    void onStopped(const QString &path);
    void onPaused();
    void onResumed();
    void onError(const QString &message);
    void onElapsedChanged(int seconds);
};
