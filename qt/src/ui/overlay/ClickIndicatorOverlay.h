#ifndef CLICKINDICATOROVERLAY_H
#define CLICKINDICATOROVERLAY_H

#include <QWidget>
#include <QObject>
#include <QPoint>
#include <QVector>
#include <QHash>
#include <QElapsedTimer>

class QTimer;
class QScreen;

// One transparent ripple window covering a single display. A single window
// spanning the union of all screens doesn't work on macOS with separate
// Spaces — the window is assigned to one display and ripples on the other
// displays are invisible. ClickIndicatorOverlay below owns one of these per
// QScreen.
class ClickRippleWindow : public QWidget {
    Q_OBJECT
public:
    explicit ClickRippleWindow(QScreen *screen, QWidget *parent = nullptr);
    ~ClickRippleWindow() override;

    void showOverlay();
    void hideOverlay();
    // globalPos is in screen (virtual desktop) coordinates; must lie on this
    // window's display.
    void addRipple(QPoint globalPos, bool rightClick);

protected:
    void paintEvent(QPaintEvent *event) override;

private:
    struct Ripple {
        QPointF localPos;
        qint64  startMs;
        bool    rightClick;
    };

    void tick();

    QScreen *m_screen = nullptr;
    QTimer *m_anim = nullptr;
    QElapsedTimer m_clock;
    QVector<Ripple> m_ripples;

    static constexpr int    kLifetimeMs = 500;
    static constexpr qreal  kStartRadius = 6.0;
    static constexpr qreal  kEndRadius   = 36.0;
};

// Manager keeping one ClickRippleWindow per QScreen, tracking display
// hot-plug via QGuiApplication::screenAdded/screenRemoved. The public API
// matches the old single-window overlay so callers (RecordingController, the
// ClickTap connection in main.cpp) are unchanged.
class ClickIndicatorOverlay : public QObject {
    Q_OBJECT
public:
    explicit ClickIndicatorOverlay(QObject *parent = nullptr);
    ~ClickIndicatorOverlay() override;

    void showOverlay();
    void hideOverlay();

public slots:
    // globalPos is in screen (virtual desktop) coordinates.
    void addRipple(QPoint globalPos, bool rightClick);

private:
    void onScreenAdded(QScreen *screen);
    void onScreenRemoved(QScreen *screen);

    QHash<QScreen *, ClickRippleWindow *> m_windows;
    bool m_visible = false;
};

#endif // CLICKINDICATOROVERLAY_H
