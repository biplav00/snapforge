#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>
#include <QList>
#include <QPair>
#include "AnnotationState.h"

class AnnotationCanvas;
class AnnotationToolbar;
class QPushButton;
class QScreen;
namespace shortcuts { class Snapshot; }

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();
    void activateFullscreen();
    void activateForRecording();
    void setRememberRegion(bool enabled) { m_rememberRegion = enabled; }
    // Seed the remembered region (restored from config at startup). Window-local
    // coords of the display it was captured on.
    void setLastRegion(const QRect &region);

    // Re-read the overlay-local shortcuts (hotkeys.tools/sizes/actions in the
    // shared config) and re-parse them into matchable chords. Called at
    // construction and after every Preferences save (see main.cpp).
    void reloadKeyBindings();

    // True only when overlay is visible AND mid-flight work exists (drawing,
    // region committed, annotate/record-select mode, or async capture pending).
    // Visible-but-idle returns false so a desynced AppKit hide can't deadlock
    // the hotkey gate in main.cpp.
    bool isBusy() const;

signals:
    void screenshotReady(QImage composited, int w, int h);
    void clipboardReady(QImage composited, int w, int h);
    void cancelled();
    void recordingRequested(int display, QRect region);
    void regionInvalid(QString reason);
    // Emitted whenever a capture commits while "remember region" is on, so the
    // region can be persisted to config and survive a restart.
    void lastRegionChanged(QRect region);

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
    // Force the (single, long-lived) overlay window onto `screen`, re-anchoring
    // the reused native window after a display reconfiguration. See the .cpp
    // for why a plain setGeometry() is not enough.
    void repositionToScreen(QScreen *screen);
    // Slot for qApp screen-add/remove/primary-change. Re-anchors the overlay if
    // it's currently visible when the display topology changes underneath it.
    void onScreenConfigChanged();
    void enterAnnotateMode();
    void exitAnnotateMode();
    void enterRecordSelectMode();
    void exitRecordSelectMode();
    void positionRecordButtons(const QRect &sel);
    void handleCopy();
    void handleSaveAndCopy();
    // Save the current region into m_last* and emit lastRegionChanged, if the
    // "remember region" pref is on. Called from every commit path.
    void rememberCurrentRegion();
    void hideOverlay();
    // Hold/release an application override crosshair for the draw-a-region phase
    // (Select mode, no region yet). Idempotent and driven purely by state, so it
    // can be called from every transition without tracking push/pop balance. See
    // the .cpp for why a per-widget cursor is not reliable on macOS.
    void syncSelectCursor();
    // Reset overlay + capture state after a failed/stalled async capture so
    // isBusy() can't latch true forever and wedge the hotkey gate.
    void handleCaptureFailure(const char *reason);
    ResizeEdge edgeAt(QPoint pos) const;
    void beginResize(ResizeEdge edge);
    void applyResize(QPoint pos);
    void emitRecordRegion();
    void emitRecordFullscreen();

    // One rebindable overlay-local shortcut, parsed from the shared config.
    // key/mods use Qt's macOS mapping (Cmd ⇒ ControlModifier) — see
    // shortcuts::toQtKey.
    struct KeyBinding {
        int key = 0;
        Qt::KeyboardModifiers mods = Qt::NoModifier;
        bool matches(const QKeyEvent *event) const;
    };
    static KeyBinding bindingFor(const shortcuts::Snapshot &cfg,
                                 const char *section, const char *actionId,
                                 const char *defaultChord);

    QList<QPair<KeyBinding, ToolType>> m_toolBindings;
    KeyBinding m_bindSizeSmall, m_bindSizeMedium, m_bindSizeLarge;
    KeyBinding m_bindSave, m_bindCopy, m_bindUndo, m_bindRedo, m_bindCancel;

    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    bool m_resizing = false;
    // True while an application override crosshair is held for the draw-region
    // phase. Guards syncSelectCursor() against unbalanced push/restore.
    bool m_selectCursorActive = false;
    ResizeEdge m_activeResize = EdgeNone;
    int m_resizeFixedLeft = 0;
    int m_resizeFixedRight = 0;
    int m_resizeFixedTop = 0;
    int m_resizeFixedBottom = 0;
    bool m_rememberRegion = false;
    QPoint m_lastStartPos;
    QPoint m_lastEndPos;
    QImage m_screenshot;
    // True from the moment an async fullscreen capture is launched until its
    // completion handler runs (success OR failure) or the watchdog fires.
    // Drives isBusy(): a pending capture is busy, a resolved one is not — even
    // if it failed — so one failed capture can't poison every later hotkey.
    bool m_captureInFlight = false;
    // Bumped once per activation. Guards stale completion/watchdog callbacks:
    // a later activation supersedes earlier ones, which must not touch state.
    quint64 m_captureSeq = 0;
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
