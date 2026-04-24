#ifndef ANNOTATIONSTATE_H
#define ANNOTATIONSTATE_H

#include <QObject>
#include <QVector>
#include <optional>
#include "Annotation.h"

class AnnotationState : public QObject {
    Q_OBJECT

public:
    explicit AnnotationState(QObject *parent = nullptr);

    const QVector<Annotation> &annotations() const { return m_annotations; }
    void commitAnnotation(const Annotation &a);
    void updateAnnotation(const QString &id, const Annotation &a);
    void clearAnnotations();
    void offsetAnnotations(double dx, double dy);

    const std::optional<Annotation> &activeAnnotation() const { return m_active; }
    void setActiveAnnotation(const Annotation &a);
    void setActiveAnnotationQuiet(const Annotation &a);  // Update without emitting changed()
    void clearActiveAnnotation();

    bool canUndo() const { return !m_undoStack.isEmpty(); }
    bool canRedo() const { return !m_redoStack.isEmpty(); }
    void undo();
    void redo();

    ToolType activeTool() const { return m_activeTool; }
    void setActiveTool(ToolType t);

    QColor strokeColor() const { return m_strokeColor; }
    void setStrokeColor(const QColor &c);

    int strokeWidth() const { return m_strokeWidth; }
    void setStrokeWidth(int w);

    int nextStepNumber() const { return m_nextStep; }
    void incrementStepNumber() { m_nextStep++; }

    static QString newId();

signals:
    void changed();
    void toolChanged(ToolType tool);
    void colorChanged(QColor color);
    void strokeWidthChanged(int width);

private:
    QVector<Annotation> m_annotations;
    std::optional<Annotation> m_active;
    ToolType m_activeTool = ToolType::Arrow;
    QColor m_strokeColor = QColor("#FF0000");
    int m_strokeWidth = 2;
    int m_nextStep = 1;

    void recalcNextStep();

    QVector<QVector<Annotation>> m_undoStack;
    QVector<QVector<Annotation>> m_redoStack;
    static constexpr int MAX_UNDO = 50;
};

#endif
