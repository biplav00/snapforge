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
    ++m_committedRevision;
    emit changed();
}

void AnnotationState::updateAnnotation(const QString &id, const Annotation &a)
{
    // Find the target first: if the id isn't present, return without touching
    // the undo/redo stacks. Snapshotting unconditionally recorded a phantom
    // undo step (first Ctrl-Z appeared to do nothing) and discarded redo
    // history on a no-op.
    int idx = -1;
    for (int i = 0; i < m_annotations.size(); ++i) {
        if (m_annotations[i].id == id) { idx = i; break; }
    }
    if (idx < 0)
        return;

    if (m_undoStack.size() >= MAX_UNDO)
        m_undoStack.removeFirst();
    m_undoStack.append(m_annotations);
    m_redoStack.clear();
    m_annotations[idx] = a;
    ++m_committedRevision;
    emit changed();
}

void AnnotationState::clearAnnotations()
{
    m_undoStack.clear();
    m_redoStack.clear();
    m_annotations.clear();
    m_active.reset();
    m_nextStep = 1;
    ++m_committedRevision;
    emit changed();
}


void AnnotationState::setActiveAnnotation(const Annotation &a)
{
    m_active = a;
    emit changed();
}

void AnnotationState::setActiveAnnotationQuiet(const Annotation &a)
{
    m_active = a;
}

void AnnotationState::clearActiveAnnotation()
{
    m_active.reset();
    emit changed();
}

void AnnotationState::recalcNextStep()
{
    int maxStep = 0;
    for (const Annotation &a : m_annotations) {
        if (a.tool == ToolType::Steps) {
            const auto &sd = std::get<StepsData>(a.data);
            if (sd.number > maxStep) maxStep = sd.number;
        }
    }
    m_nextStep = maxStep + 1;
}

void AnnotationState::undo()
{
    if (m_undoStack.isEmpty())
        return;
    m_redoStack.append(m_annotations);
    m_annotations = m_undoStack.takeLast();
    recalcNextStep();
    ++m_committedRevision;
    emit changed();
}

void AnnotationState::redo()
{
    if (m_redoStack.isEmpty())
        return;
    m_undoStack.append(m_annotations);
    m_annotations = m_redoStack.takeLast();
    recalcNextStep();
    ++m_committedRevision;
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
