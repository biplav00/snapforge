#ifndef ANNOTATIONCANVAS_H
#define ANNOTATIONCANVAS_H

#include <QWidget>
#include <QImage>
#include <QLineEdit>
#include <QElapsedTimer>
#include "Annotation.h"
#include "AnnotationState.h"

class AnnotationCanvas : public QWidget {
    Q_OBJECT

public:
    explicit AnnotationCanvas(AnnotationState *state, const QImage &screenshot, QWidget *parent = nullptr);

    void setScreenshot(const QImage &screenshot);
    void setRegion(int x, int y, int w, int h);

    QImage compositeImage() const;

    bool isTextInputActive() const { return m_waitingForText; }

protected:
    void resizeEvent(QResizeEvent *event) override;

public slots:
    // Discard any pending text input. Safe to call if no input is active.
    void cancelTextInput();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;
    void hideEvent(QHideEvent *event) override;

private:
    void showTextInput(int x, int y);
    void commitTextInput();

    // Returns the DPR of the screen this canvas is on (falling back to the
    // global helper), used both for compositing and the committed-layer cache.
    double effectiveDpr() const;

    // Lazily (re)build the cached image of all committed annotations. The
    // cache lets paintEvent skip re-rendering the whole committed-annotation
    // list on every frame — during a freehand drag only the in-progress
    // active annotation is drawn on top of the cached layer.
    void ensureCommittedLayer() const;

    AnnotationState *m_state;
    QImage m_fullScreenshot;
    QImage m_croppedScreenshot;

    // Cached render of every committed annotation. Stored at device-pixel size
    // (widget_size * dpr) with devicePixelRatio set so it blits 1:1 in logical
    // coords. Invalidated whenever the committed set, size, or DPR changes.
    mutable QImage m_committedLayer;
    mutable quint64 m_committedLayerRevision = 0;
    mutable bool m_committedLayerValid = false;
    mutable double m_committedLayerDpr = 0.0;

    // Drag state
    bool m_dragging = false;
    QPointF m_dragStart;
    QElapsedTimer m_lastMoveUpdate;
    bool m_moveTimerStarted = false;

    // Text/Steps input
    bool m_waitingForText = false;
    QLineEdit *m_lineEdit = nullptr;
    QPointF m_textPos;         // Where the text/step annotation will be placed
    int m_pendingStepNumber = 0;
    bool m_pendingIsSteps = false;
    QString m_editingStepId;   // Non-empty when editing label of existing step
    QColor m_editingStepColor;
    int m_editingStepStrokeWidth = 0;
};

#endif // ANNOTATIONCANVAS_H
