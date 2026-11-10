#ifndef RECORDINGBORDERWINDOW_H
#define RECORDINGBORDERWINDOW_H

#include <QRect>
#include <QWidget>

// A click-through outline drawn around the region currently being recorded.
//
// The recording itself crops to the region inside this outline; the outline is
// deliberately drawn just OUTSIDE those pixels so it never covers recorded
// content. On macOS the native window is marked NSWindowSharingNone, so
// ScreenCaptureKit (and the CG screenshot fallback) does not capture it.
//
// Owned by main and driven by RecordingManager's lifecycle:
//   setRegion()            when the overlay commits a region
//   showForPreparedRegion() on recordingStarted
//   setPaused()            on recordingPaused/Resumed
//   clear()                on finalizing/stopped/error
class RecordingBorderWindow : public QWidget {
    Q_OBJECT
public:
    explicit RecordingBorderWindow(QWidget *parent = nullptr);

    // Prepare the outline for a region in GLOBAL logical coordinates. An empty
    // rect means a fullscreen recording, which needs no outline.
    void setRegion(const QRect &globalRect);

    // Show the prepared outline. No-op when no region was prepared.
    void showForPreparedRegion();

    // Dashed/fainter outline while the recording is paused.
    void setPaused(bool paused);

    // Drop the prepared region and hide the outline.
    void clear();

protected:
    void paintEvent(QPaintEvent *event) override;

private:
    void applyNativeWindowStyle();

    QRect m_region; // captured region, global logical coords
    bool m_paused = false;
    bool m_nativeConfigured = false;
};

#endif // RECORDINGBORDERWINDOW_H
