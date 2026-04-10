#include "AnnotationCanvas.h"
#include "AnnotationRenderer.h"

#include <QPainter>
#include <QPaintEvent>
#include <QMouseEvent>
#include <QKeyEvent>
#include <cmath>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static inline double dist2(QPointF a, QPointF b) {
    double dx = b.x() - a.x();
    double dy = b.y() - a.y();
    return std::sqrt(dx * dx + dy * dy);
}

static inline double rectDiag(double w, double h) {
    return std::sqrt(w * w + h * h);
}

// Normalise a rect so width/height are always positive
static void normaliseRect(double &x, double &y, double &w, double &h) {
    if (w < 0) { x += w; w = -w; }
    if (h < 0) { y += h; h = -h; }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

AnnotationCanvas::AnnotationCanvas(AnnotationState *state,
                                   const QImage &screenshot,
                                   QWidget *parent)
    : QWidget(parent)
    , m_state(state)
    , m_fullScreenshot(screenshot)
{
    setAttribute(Qt::WA_TransparentForMouseEvents, false);
    setAttribute(Qt::WA_NoSystemBackground, true);
    setAttribute(Qt::WA_TranslucentBackground, true);
    setMouseTracking(true);

    connect(m_state, &AnnotationState::changed, this, [this]() { update(); });
}

// ---------------------------------------------------------------------------
// setRegion
// ---------------------------------------------------------------------------

void AnnotationCanvas::setRegion(int x, int y, int w, int h) {
    setGeometry(x, y, w, h);

    if (!m_fullScreenshot.isNull()) {
        m_croppedScreenshot = m_fullScreenshot.copy(x, y, w, h);
    }
}

// ---------------------------------------------------------------------------
// compositeImage
// ---------------------------------------------------------------------------

QImage AnnotationCanvas::compositeImage() const {
    QImage result = m_croppedScreenshot.isNull()
                    ? QImage(size(), QImage::Format_ARGB32_Premultiplied)
                    : m_croppedScreenshot.convertToFormat(QImage::Format_ARGB32_Premultiplied);

    if (result.isNull()) {
        result = QImage(size(), QImage::Format_ARGB32_Premultiplied);
        result.fill(Qt::transparent);
    }

    QPainter p(&result);
    p.setRenderHint(QPainter::Antialiasing, true);

    for (const Annotation &a : m_state->annotations()) {
        AnnotationRenderer::render(p, a, m_croppedScreenshot);
    }

    if (m_state->activeAnnotation().has_value()) {
        AnnotationRenderer::render(p, m_state->activeAnnotation().value(), m_croppedScreenshot);
    }

    return result;
}

// ---------------------------------------------------------------------------
// paintEvent
// ---------------------------------------------------------------------------

void AnnotationCanvas::paintEvent(QPaintEvent * /*event*/) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);

    const ToolType active = m_state->activeTool();

    // Committed annotations
    for (const Annotation &a : m_state->annotations()) {
        AnnotationRenderer::render(p, a, m_croppedScreenshot);
    }

    // Active (in-progress) annotation — skip Text/Steps while editing
    if (m_state->activeAnnotation().has_value()) {
        const Annotation &ann = m_state->activeAnnotation().value();
        bool isTextOrSteps = (active == ToolType::Text || active == ToolType::Steps);
        if (!isTextOrSteps) {
            AnnotationRenderer::render(p, ann, m_croppedScreenshot);
        }
    }
}

// ---------------------------------------------------------------------------
// Mouse events
// ---------------------------------------------------------------------------

void AnnotationCanvas::mousePressEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) { return; }

    const QPointF pos = event->position();
    const ToolType tool = m_state->activeTool();

    // --- ColorPicker ---
    if (tool == ToolType::ColorPicker) {
        if (!m_croppedScreenshot.isNull()) {
            int px = static_cast<int>(pos.x());
            int py = static_cast<int>(pos.y());
            px = std::max(0, std::min(px, m_croppedScreenshot.width() - 1));
            py = std::max(0, std::min(py, m_croppedScreenshot.height() - 1));
            QColor sampled = m_croppedScreenshot.pixelColor(px, py);
            m_state->setStrokeColor(sampled);
        }
        return;
    }

    // --- Text ---
    if (tool == ToolType::Text) {
        m_pendingIsSteps = false;
        m_textPos = pos;
        showTextInput(static_cast<int>(pos.x()), static_cast<int>(pos.y()));
        return;
    }

    // --- Steps ---
    if (tool == ToolType::Steps) {
        m_pendingIsSteps = true;
        m_pendingStepNumber = m_state->nextStepNumber();
        m_textPos = pos;
        // Show QLineEdit offset from the step circle (x+28, y-12)
        showTextInput(static_cast<int>(pos.x()) + 28, static_cast<int>(pos.y()) - 12);
        return;
    }

    // --- All drag tools ---
    m_dragging = true;
    m_dragStart = pos;

    // Create initial annotation at the click point
    Annotation ann;
    ann.id = AnnotationState::newId();
    ann.tool = tool;
    ann.color = m_state->strokeColor();
    ann.strokeWidth = m_state->strokeWidth();

    switch (tool) {
    case ToolType::Arrow:
        ann.data = ArrowData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::Rect:
        ann.data = RectData{pos.x(), pos.y(), 0.0, 0.0};
        break;
    case ToolType::Circle:
        ann.data = CircleData{pos.x(), pos.y(), 0.0, 0.0};
        break;
    case ToolType::Line:
        ann.data = LineData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::DottedLine:
        ann.data = DottedLineData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::Freehand:
        ann.data = FreehandData{{pos}};
        break;
    case ToolType::Highlight:
        ann.data = HighlightData{pos.x(), pos.y(), 0.0, 0.0};
        break;
    case ToolType::Blur:
        ann.data = BlurData{pos.x(), pos.y(), 0.0, 0.0, 10};
        break;
    case ToolType::Measure:
        ann.data = MeasureData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    default:
        m_dragging = false;
        return;
    }

    m_state->setActiveAnnotation(ann);
}

void AnnotationCanvas::mouseMoveEvent(QMouseEvent *event) {
    if (!m_dragging) { return; }
    if (!m_state->activeAnnotation().has_value()) { return; }

    const QPointF pos = event->position();
    Annotation ann = m_state->activeAnnotation().value();

    std::visit([&](auto &d) {
        using T = std::decay_t<decltype(d)>;

        if constexpr (std::is_same_v<T, ArrowData> ||
                      std::is_same_v<T, LineData> ||
                      std::is_same_v<T, DottedLineData> ||
                      std::is_same_v<T, MeasureData>) {
            d.endX = pos.x();
            d.endY = pos.y();
        } else if constexpr (std::is_same_v<T, RectData> ||
                             std::is_same_v<T, HighlightData>) {
            d.width  = pos.x() - m_dragStart.x();
            d.height = pos.y() - m_dragStart.y();
        } else if constexpr (std::is_same_v<T, BlurData>) {
            d.width  = pos.x() - m_dragStart.x();
            d.height = pos.y() - m_dragStart.y();
        } else if constexpr (std::is_same_v<T, CircleData>) {
            d.rx = std::abs(pos.x() - m_dragStart.x()) / 2.0;
            d.ry = std::abs(pos.y() - m_dragStart.y()) / 2.0;
            // Keep center at midpoint of drag
            d.cx = m_dragStart.x() + (pos.x() - m_dragStart.x()) / 2.0;
            d.cy = m_dragStart.y() + (pos.y() - m_dragStart.y()) / 2.0;
        } else if constexpr (std::is_same_v<T, FreehandData>) {
            d.points.append(pos);
        }
    }, ann.data);

    m_state->setActiveAnnotation(ann);
}

void AnnotationCanvas::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) { return; }
    if (!m_dragging) { return; }
    m_dragging = false;

    if (!m_state->activeAnnotation().has_value()) { return; }

    Annotation ann = m_state->activeAnnotation().value();
    bool valid = false;

    std::visit([&](auto &d) {
        using T = std::decay_t<decltype(d)>;

        if constexpr (std::is_same_v<T, ArrowData> ||
                      std::is_same_v<T, LineData> ||
                      std::is_same_v<T, DottedLineData> ||
                      std::is_same_v<T, MeasureData>) {
            double distance = dist2(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));
            valid = (distance > 3.0);
        } else if constexpr (std::is_same_v<T, RectData> ||
                             std::is_same_v<T, HighlightData>) {
            // Normalise negative dimensions
            normaliseRect(d.x, d.y, d.width, d.height);
            valid = (rectDiag(d.width, d.height) > 3.0);
        } else if constexpr (std::is_same_v<T, BlurData>) {
            normaliseRect(d.x, d.y, d.width, d.height);
            valid = (rectDiag(d.width, d.height) > 3.0);
        } else if constexpr (std::is_same_v<T, CircleData>) {
            valid = (d.rx > 3.0 || d.ry > 3.0);
        } else if constexpr (std::is_same_v<T, FreehandData>) {
            valid = (d.points.size() >= 2);
        }
    }, ann.data);

    if (valid) {
        m_state->commitAnnotation(ann);
    } else {
        m_state->clearActiveAnnotation();
    }
}

// ---------------------------------------------------------------------------
// Text input
// ---------------------------------------------------------------------------

void AnnotationCanvas::showTextInput(int x, int y) {
    cancelTextInput(); // clean up any existing input

    m_waitingForText = true;

    m_lineEdit = new QLineEdit(this);
    m_lineEdit->setMinimumWidth(140);
    m_lineEdit->resize(160, 28);
    m_lineEdit->move(x, y);

    m_lineEdit->setStyleSheet(
        "QLineEdit {"
        "  background: rgba(30, 30, 30, 210);"
        "  border: 1px dashed white;"
        "  border-radius: 3px;"
        "  color: white;"
        "  padding: 2px 6px;"
        "  font-size: 13px;"
        "}"
    );

    m_lineEdit->setPlaceholderText(m_pendingIsSteps ? "Label (optional)" : "Type text…");
    m_lineEdit->installEventFilter(this);
    m_lineEdit->show();
    m_lineEdit->setFocus();

    connect(m_lineEdit, &QLineEdit::returnPressed, this, &AnnotationCanvas::commitTextInput);
}

void AnnotationCanvas::commitTextInput() {
    if (!m_lineEdit || !m_waitingForText) { return; }

    QString text = m_lineEdit->text();
    m_waitingForText = false;

    Annotation ann;
    ann.id = AnnotationState::newId();
    ann.color = m_state->strokeColor();
    ann.strokeWidth = m_state->strokeWidth();

    if (m_pendingIsSteps) {
        ann.tool = ToolType::Steps;
        ann.data = StepsData{m_textPos.x(), m_textPos.y(), m_pendingStepNumber, text};
        m_state->commitAnnotation(ann);
        m_state->incrementStepNumber();
    } else {
        if (text.isEmpty()) {
            // Discard empty text annotation
        } else {
            ann.tool = ToolType::Text;
            ann.data = TextData{m_textPos.x(), m_textPos.y(), text, 16};
            m_state->commitAnnotation(ann);
        }
    }

    m_lineEdit->deleteLater();
    m_lineEdit = nullptr;
    update();
}

void AnnotationCanvas::cancelTextInput() {
    if (!m_lineEdit) { return; }
    m_waitingForText = false;
    m_lineEdit->deleteLater();
    m_lineEdit = nullptr;
    m_state->clearActiveAnnotation();
    update();
}

// ---------------------------------------------------------------------------
// eventFilter — catch Escape in the QLineEdit
// ---------------------------------------------------------------------------

bool AnnotationCanvas::eventFilter(QObject *watched, QEvent *event) {
    if (watched == m_lineEdit && event->type() == QEvent::KeyPress) {
        QKeyEvent *ke = static_cast<QKeyEvent *>(event);
        if (ke->key() == Qt::Key_Escape) {
            cancelTextInput();
            return true;
        }
    }
    return QWidget::eventFilter(watched, event);
}
