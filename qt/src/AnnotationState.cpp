#include "AnnotationState.h"
#include <QUuid>

AnnotationState::AnnotationState(QObject *parent)
    : QObject(parent)
{
}

void AnnotationState::commitAnnotation(const Annotation &a)
{
    if (m_undoStack.size() >= MAX_UNDO)
        m_undoStack.removeFirst();
    m_undoStack.append(m_annotations);
    m_redoStack.clear();
    m_annotations.append(a);
    m_active.reset();
    emit changed();
}

void AnnotationState::clearAnnotations()
{
    m_undoStack.clear();
    m_redoStack.clear();
    m_annotations.clear();
    m_active.reset();
    m_nextStep = 1;
    emit changed();
}

void AnnotationState::offsetAnnotations(double dx, double dy)
{
    for (Annotation &ann : m_annotations) {
        std::visit([dx, dy](auto &d) {
            using T = std::decay_t<decltype(d)>;
            if constexpr (std::is_same_v<T, ArrowData> ||
                          std::is_same_v<T, LineData> ||
                          std::is_same_v<T, DottedLineData> ||
                          std::is_same_v<T, MeasureData>) {
                d.startX += dx;
                d.startY += dy;
                d.endX   += dx;
                d.endY   += dy;
            } else if constexpr (std::is_same_v<T, RectData> ||
                                 std::is_same_v<T, HighlightData> ||
                                 std::is_same_v<T, BlurData>) {
                d.x += dx;
                d.y += dy;
            } else if constexpr (std::is_same_v<T, CircleData>) {
                d.cx += dx;
                d.cy += dy;
            } else if constexpr (std::is_same_v<T, FreehandData>) {
                for (QPointF &pt : d.points) {
                    pt.rx() += dx;
                    pt.ry() += dy;
                }
            } else if constexpr (std::is_same_v<T, TextData> ||
                                 std::is_same_v<T, StepsData> ||
                                 std::is_same_v<T, ColorPickerData>) {
                d.x += dx;
                d.y += dy;
            }
        }, ann.data);
    }
    emit changed();
}

void AnnotationState::setActiveAnnotation(const Annotation &a)
{
    m_active = a;
    emit changed();
}

void AnnotationState::clearActiveAnnotation()
{
    m_active.reset();
    emit changed();
}

void AnnotationState::undo()
{
    if (m_undoStack.isEmpty())
        return;
    m_redoStack.append(m_annotations);
    m_annotations = m_undoStack.takeLast();
    emit changed();
}

void AnnotationState::redo()
{
    if (m_redoStack.isEmpty())
        return;
    m_undoStack.append(m_annotations);
    m_annotations = m_redoStack.takeLast();
    emit changed();
}

void AnnotationState::setActiveTool(ToolType t)
{
    m_activeTool = t;
    emit toolChanged(t);
}

void AnnotationState::setStrokeColor(const QColor &c)
{
    m_strokeColor = c;
    emit colorChanged(c);
}

void AnnotationState::setStrokeWidth(int w)
{
    m_strokeWidth = w;
    emit strokeWidthChanged(w);
}

QString AnnotationState::newId()
{
    return QUuid::createUuid().toString(QUuid::WithoutBraces);
}
