#ifndef CLICKINDICATOROVERLAY_H
#define CLICKINDICATOROVERLAY_H

#include <QWidget>
#include <QPoint>
#include <QVector>
#include <QElapsedTimer>

class QTimer;

class ClickIndicatorOverlay : public QWidget {
    Q_OBJECT
public:
    explicit ClickIndicatorOverlay(QWidget *parent = nullptr);
    ~ClickIndicatorOverlay() override;

    void showOverlay();
    void hideOverlay();

public slots:
    // globalPos is in screen (virtual desktop) coordinates.
    void addRipple(QPoint globalPos, bool rightClick);

protected:
    void paintEvent(QPaintEvent *event) override;

private:
    struct Ripple {
        QPointF localPos;
        qint64  startMs;
        bool    rightClick;
    };

    void resizeToVirtualDesktop();
    void tick();

    QTimer *m_anim = nullptr;
    QElapsedTimer m_clock;
    QVector<Ripple> m_ripples;

    static constexpr int    kLifetimeMs = 500;
    static constexpr qreal  kStartRadius = 6.0;
    static constexpr qreal  kEndRadius   = 36.0;
};

#endif // CLICKINDICATOROVERLAY_H
