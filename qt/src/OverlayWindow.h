#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>
#include <QThread>
#include <QMap>
#include "AnnotationState.h"

class AnnotationCanvas;
class AnnotationToolbar;

class CaptureWorker : public QThread {
    Q_OBJECT
public:
    void run() override;
signals:
    void captured(QImage image);
};

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();

signals:
    void screenshotReady(QImage composited, int w, int h);
    void clipboardReady(QImage composited, int w, int h);
    void cancelled();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    enum Mode { Select, Annotate };

    void enterAnnotateMode();
    void exitAnnotateMode();
    void handleSave();
    void handleCopy();
    void hideOverlay();
    bool isOnRegionEdge(QPoint pos) const;

    static const QMap<int, ToolType> &toolShortcuts();

    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    QImage m_screenshot;
    CaptureWorker m_captureWorker;
    Mode m_mode = Select;

    AnnotationState m_annotationState;
    AnnotationCanvas *m_canvas = nullptr;
    AnnotationToolbar *m_toolbar = nullptr;

    QRect selectedRect() const;
};

#endif // OVERLAYWINDOW_H
