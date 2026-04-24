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

    void setRegion(int x, int y, int w, int h);

    QImage compositeImage() const;

    bool isTextInputActive() const { return m_waitingForText; }

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

    AnnotationState *m_state;
    QImage m_fullScreenshot;
    QImage m_croppedScreenshot;

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
