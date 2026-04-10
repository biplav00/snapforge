#ifndef ANNOTATIONCANVAS_H
#define ANNOTATIONCANVAS_H

#include <QWidget>
#include <QImage>
#include <QLineEdit>
#include "Annotation.h"
#include "AnnotationState.h"

class AnnotationCanvas : public QWidget {
    Q_OBJECT

public:
    explicit AnnotationCanvas(AnnotationState *state, const QImage &screenshot, QWidget *parent = nullptr);

    void setRegion(int x, int y, int w, int h);

    QImage compositeImage() const;

    bool isTextInputActive() const { return m_waitingForText; }

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    void showTextInput(int x, int y);
    void commitTextInput();
    void cancelTextInput();

    AnnotationState *m_state;
    QImage m_fullScreenshot;
    QImage m_croppedScreenshot;

    // Drag state
    bool m_dragging = false;
    QPointF m_dragStart;

    // Text/Steps input
    bool m_waitingForText = false;
    QLineEdit *m_lineEdit = nullptr;
    QPointF m_textPos;         // Where the text/step annotation will be placed
    int m_pendingStepNumber = 0;
    bool m_pendingIsSteps = false;
};

#endif // ANNOTATIONCANVAS_H
