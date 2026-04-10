#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();

signals:
    void regionCaptured(int x, int y, int w, int h);
    void cancelled();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    QImage m_screenshot;

    QRect selectedRect() const;
};

#endif // OVERLAYWINDOW_H
