#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>
#include <QMap>
#include "AnnotationState.h"

class AnnotationCanvas;
class AnnotationToolbar;
class QPushButton;

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();
    void activateForRecording();

signals:
    void screenshotReady(QImage composited, int w, int h);
    void clipboardReady(QImage composited, int w, int h);
    void cancelled();
    void recordingRequested(int display, QRect region);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    enum Mode { Select, Annotate, RecordSelect };
    enum Purpose { Screenshot, Record };

    void activateInternal();
    void enterAnnotateMode();
    void exitAnnotateMode();
    void enterRecordSelectMode();
    void exitRecordSelectMode();
    void handleSave();
    void handleCopy();
    void hideOverlay();
    bool isOnRegionEdge(QPoint pos) const;
    void emitRecordRegion();
    void emitRecordFullscreen();

    static const QMap<int, ToolType> &toolShortcuts();

    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    QImage m_screenshot;
    Mode m_mode = Select;
    Purpose m_purpose = Screenshot;

    AnnotationState m_annotationState;
    AnnotationCanvas *m_canvas = nullptr;
    AnnotationToolbar *m_toolbar = nullptr;

    // Record-select mode buttons
    QPushButton *m_btnRecordRegion   = nullptr;
    QPushButton *m_btnRecordFullscreen = nullptr;
    QPushButton *m_btnRecordCancel   = nullptr;

    QRect selectedRect() const;
};

#endif // OVERLAYWINDOW_H
