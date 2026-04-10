#ifndef ANNOTATIONTOOLBAR_H
#define ANNOTATIONTOOLBAR_H

#include <QWidget>
#include <QColor>
#include "Annotation.h"

class AnnotationState;
class QAbstractButton;
class QPushButton;

class AnnotationToolbar : public QWidget {
    Q_OBJECT

public:
    explicit AnnotationToolbar(AnnotationState *state, QWidget *parent = nullptr);

    void positionRelativeTo(int regionX, int regionY, int regionW, int regionH);

signals:
    void saveRequested();
    void copyRequested();
    void cancelRequested();

protected:
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void paintEvent(QPaintEvent *event) override;

private:
    void buildUi();
    void updateToolButtons();
    void updateUndoRedo();
    void updateColorSwatches();
    void onColorSwatchClicked(const QColor &color);
    void onCustomColorClicked();
    void onStrokeWidthClicked(int width);

    AnnotationState *m_state;

    // Tool buttons in order matching ToolType enum
    QList<QPushButton *> m_toolButtons;

    // Color swatch buttons
    QList<QPushButton *> m_colorSwatches;
    static const QList<QColor> k_presetColors;

    // Stroke width buttons
    QList<QPushButton *> m_strokeButtons;
    static const QList<int> k_strokeWidths;

    // Undo / Redo
    QPushButton *m_undoBtn = nullptr;
    QPushButton *m_redoBtn = nullptr;

    // Drag state
    bool m_dragging = false;
    QPoint m_dragStartPos;
    QPoint m_dragStartGlobal;
};

#endif // ANNOTATIONTOOLBAR_H
