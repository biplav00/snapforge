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
    void activateFullscreen();
    void activateForRecording();
    void setRememberRegion(bool enabled) { m_rememberRegion = enabled; }

signals:
    void screenshotReady(QImage composited, int w, int h);
    void clipboardReady(QImage composited, int w, int h);
    void cancelled();
    void recordingRequested(int display, QRect region);
    void regionInvalid(QString reason);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    enum Mode { Select, Annotate, RecordSelect };
    enum Purpose { Screenshot, Record };
    enum ResizeEdge {
        EdgeNone,
        EdgeTop,
        EdgeBottom,
        EdgeLeft,
        EdgeRight,
        EdgeTopLeft,
        EdgeTopRight,
        EdgeBottomLeft,
        EdgeBottomRight,
    };

    void activateInternal();
    void enterAnnotateMode();
    void exitAnnotateMode();
    void enterRecordSelectMode();
    void exitRecordSelectMode();
    void handleSave();
    void handleCopy();
    void handleSaveAndCopy();
    void hideOverlay();
    bool isOnRegionEdge(QPoint pos) const;
    ResizeEdge edgeAt(QPoint pos) const;
    void beginResize(ResizeEdge edge);
    void applyResize(QPoint pos);
    void emitRecordRegion();
    void emitRecordFullscreen();

    static const QMap<int, ToolType> &toolShortcuts();

    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    bool m_resizing = false;
    ResizeEdge m_activeResize = EdgeNone;
    int m_resizeFixedLeft = 0;
    int m_resizeFixedRight = 0;
    int m_resizeFixedTop = 0;
    int m_resizeFixedBottom = 0;
    bool m_rememberRegion = false;
    QPoint m_lastStartPos;
    QPoint m_lastEndPos;
    QImage m_screenshot;
    QImage m_scaledScreenshot; // cached widget-size copy for paintEvent
    Mode m_mode = Select;
    Purpose m_purpose = Screenshot;
    int m_displayIndex = 0; // which display this overlay captures from

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
