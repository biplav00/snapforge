# Annotation Layer + Toolbar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the 12-tool annotation system and floating toolbar in Qt, enabling screenshot annotation with arrows, shapes, text, blur, and more — all rendered via QPainter and composited onto the captured image for save/copy.

**Architecture:** Annotation data types and state management are separate from rendering. Each tool type stores its data in a variant-based `Annotation` struct. A central `AnnotationState` manages the annotation list, undo/redo, active tool, and colors. `AnnotationRenderer` handles all QPainter drawing. The `AnnotationCanvas` widget sits over the selected region and delegates mouse events to tool handlers. The `AnnotationToolbar` is a floating draggable widget with tool/color/size/action buttons.

**Tech Stack:** C++17, Qt6 (Widgets, Gui), QPainter, QImage

---

## File Structure

```
qt/src/
├── Annotation.h                (data types: ToolType enum, per-tool data structs, Annotation variant)
├── AnnotationState.h           (state class: annotation list, undo/redo, active tool, color, stroke)
├── AnnotationState.cpp
├── AnnotationRenderer.h        (static renderAnnotation function)
├── AnnotationRenderer.cpp
├── AnnotationCanvas.h          (QWidget over selected region, mouse/paint events)
├── AnnotationCanvas.cpp
├── AnnotationToolbar.h         (floating draggable toolbar)
├── AnnotationToolbar.cpp
├── OverlayWindow.h             (modify: add annotate mode, integrate canvas+toolbar)
├── OverlayWindow.cpp
├── main.cpp                    (modify: compositing on save/copy)
└── CMakeLists.txt              (modify: add new source files)
```

---

### Task 1: Annotation data types

**Files:**
- Create: `qt/src/Annotation.h`

- [ ] **Step 1: Create the annotation data types header**

Create `qt/src/Annotation.h`:

```cpp
#ifndef ANNOTATION_H
#define ANNOTATION_H

#include <QString>
#include <QColor>
#include <QPointF>
#include <QVector>
#include <variant>

enum class ToolType {
    Arrow, Rect, Circle, Line, DottedLine, Freehand,
    Text, Highlight, Blur, Steps, ColorPicker, Measure
};

struct ArrowData {
    double startX, startY, endX, endY;
};

struct RectData {
    double x, y, width, height;
};

struct CircleData {
    double cx, cy, rx, ry;
};

struct LineData {
    double startX, startY, endX, endY;
};

struct DottedLineData {
    double startX, startY, endX, endY;
};

struct FreehandData {
    QVector<QPointF> points;
};

struct TextData {
    double x, y;
    QString text;
    int fontSize = 16;
};

struct HighlightData {
    double x, y, width, height;
};

struct BlurData {
    double x, y, width, height;
    int intensity = 10;
};

struct StepsData {
    double x, y;
    int number;
    QString label;
};

struct ColorPickerData {
    double x, y;
};

struct MeasureData {
    double startX, startY, endX, endY;
};

using AnnotationData = std::variant<
    ArrowData, RectData, CircleData, LineData, DottedLineData,
    FreehandData, TextData, HighlightData, BlurData, StepsData,
    ColorPickerData, MeasureData
>;

struct Annotation {
    QString id;
    ToolType tool;
    QColor color;
    int strokeWidth;
    AnnotationData data;
};

#endif // ANNOTATION_H
```

- [ ] **Step 2: Update CMakeLists.txt — no new sources needed (header-only), but verify build**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake --build build
```

Expected: Build succeeds (header not yet included anywhere).

- [ ] **Step 3: Commit**

```bash
git add qt/src/Annotation.h
git commit -m "feat(qt): add annotation data types with variant-based storage"
```

---

### Task 2: Annotation state management

**Files:**
- Create: `qt/src/AnnotationState.h`
- Create: `qt/src/AnnotationState.cpp`
- Modify: `qt/CMakeLists.txt`

- [ ] **Step 1: Create AnnotationState header**

Create `qt/src/AnnotationState.h`:

```cpp
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

    // Annotation list
    const QVector<Annotation> &annotations() const { return m_annotations; }
    void commitAnnotation(const Annotation &a);
    void clearAnnotations();
    void offsetAnnotations(double dx, double dy);

    // Active annotation (being drawn)
    const std::optional<Annotation> &activeAnnotation() const { return m_active; }
    void setActiveAnnotation(const Annotation &a);
    void clearActiveAnnotation();

    // Undo / redo
    bool canUndo() const { return !m_undoStack.isEmpty(); }
    bool canRedo() const { return !m_redoStack.isEmpty(); }
    void undo();
    void redo();

    // Tool
    ToolType activeTool() const { return m_activeTool; }
    void setActiveTool(ToolType t);

    // Color
    QColor strokeColor() const { return m_strokeColor; }
    void setStrokeColor(const QColor &c);

    // Stroke width
    int strokeWidth() const { return m_strokeWidth; }
    void setStrokeWidth(int w);

    // Steps counter
    int nextStepNumber() const { return m_nextStep; }
    void incrementStepNumber() { m_nextStep++; }

    // Generate unique ID
    static QString newId();

signals:
    void changed();           // any state change that needs repaint
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

    QVector<QVector<Annotation>> m_undoStack;
    QVector<QVector<Annotation>> m_redoStack;
    static constexpr int MAX_UNDO = 50;
};

#endif // ANNOTATIONSTATE_H
```

- [ ] **Step 2: Create AnnotationState implementation**

Create `qt/src/AnnotationState.cpp`:

```cpp
#include "AnnotationState.h"
#include <QUuid>

AnnotationState::AnnotationState(QObject *parent)
    : QObject(parent)
{
}

void AnnotationState::commitAnnotation(const Annotation &a) {
    // Push current state to undo stack
    if (m_undoStack.size() >= MAX_UNDO) {
        m_undoStack.removeFirst();
    }
    m_undoStack.push_back(m_annotations);
    m_redoStack.clear();

    m_annotations.push_back(a);
    m_active.reset();
    emit changed();
}

void AnnotationState::clearAnnotations() {
    m_annotations.clear();
    m_active.reset();
    m_undoStack.clear();
    m_redoStack.clear();
    m_nextStep = 1;
    emit changed();
}

void AnnotationState::offsetAnnotations(double dx, double dy) {
    for (auto &a : m_annotations) {
        std::visit([dx, dy](auto &d) {
            using T = std::decay_t<decltype(d)>;
            if constexpr (std::is_same_v<T, ArrowData> || std::is_same_v<T, LineData> ||
                          std::is_same_v<T, DottedLineData> || std::is_same_v<T, MeasureData>) {
                d.startX += dx; d.startY += dy;
                d.endX += dx; d.endY += dy;
            } else if constexpr (std::is_same_v<T, RectData> || std::is_same_v<T, HighlightData> ||
                                 std::is_same_v<T, BlurData>) {
                d.x += dx; d.y += dy;
            } else if constexpr (std::is_same_v<T, CircleData>) {
                d.cx += dx; d.cy += dy;
            } else if constexpr (std::is_same_v<T, FreehandData>) {
                for (auto &pt : d.points) {
                    pt.setX(pt.x() + dx);
                    pt.setY(pt.y() + dy);
                }
            } else if constexpr (std::is_same_v<T, TextData> || std::is_same_v<T, StepsData> ||
                                 std::is_same_v<T, ColorPickerData>) {
                d.x += dx; d.y += dy;
            }
        }, a.data);
    }
    emit changed();
}

void AnnotationState::setActiveAnnotation(const Annotation &a) {
    m_active = a;
    emit changed();
}

void AnnotationState::clearActiveAnnotation() {
    m_active.reset();
    emit changed();
}

void AnnotationState::undo() {
    if (m_undoStack.isEmpty()) return;
    m_redoStack.push_back(m_annotations);
    m_annotations = m_undoStack.takeLast();
    emit changed();
}

void AnnotationState::redo() {
    if (m_redoStack.isEmpty()) return;
    m_undoStack.push_back(m_annotations);
    m_annotations = m_redoStack.takeLast();
    emit changed();
}

void AnnotationState::setActiveTool(ToolType t) {
    m_activeTool = t;
    emit toolChanged(t);
}

void AnnotationState::setStrokeColor(const QColor &c) {
    m_strokeColor = c;
    emit colorChanged(c);
}

void AnnotationState::setStrokeWidth(int w) {
    m_strokeWidth = w;
    emit strokeWidthChanged(w);
}

QString AnnotationState::newId() {
    return QUuid::createUuid().toString(QUuid::WithoutBraces);
}
```

- [ ] **Step 3: Add AnnotationState.cpp to CMakeLists.txt**

In `qt/CMakeLists.txt`, update the `add_executable` block:

```cmake
add_executable(snapforge-qt
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
)
```

- [ ] **Step 4: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add qt/src/AnnotationState.h qt/src/AnnotationState.cpp qt/CMakeLists.txt
git commit -m "feat(qt): add AnnotationState with undo/redo and tool management"
```

---

### Task 3: Annotation renderer — basic tools (arrow, rect, circle, line, dotted line, freehand)

**Files:**
- Create: `qt/src/AnnotationRenderer.h`
- Create: `qt/src/AnnotationRenderer.cpp`
- Modify: `qt/CMakeLists.txt`

- [ ] **Step 1: Create renderer header**

Create `qt/src/AnnotationRenderer.h`:

```cpp
#ifndef ANNOTATIONRENDERER_H
#define ANNOTATIONRENDERER_H

#include <QPainter>
#include <QImage>
#include "Annotation.h"

namespace AnnotationRenderer {

// Render a single annotation onto the given painter.
// screenshotRegion is needed for blur tool (pixel sampling).
void render(QPainter &p, const Annotation &a, const QImage &screenshotRegion = QImage());

}

#endif // ANNOTATIONRENDERER_H
```

- [ ] **Step 2: Create renderer implementation with basic tools**

Create `qt/src/AnnotationRenderer.cpp`:

```cpp
#include "AnnotationRenderer.h"
#include <QPainterPath>
#include <QFontMetrics>
#include <cmath>

namespace AnnotationRenderer {

static void renderArrow(QPainter &p, const ArrowData &d, const QColor &color, int sw) {
    QPen pen(color, sw, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));

    // Arrowhead
    double headLen = std::max(10.0, sw * 4.0);
    double angle = std::atan2(d.endY - d.startY, d.endX - d.startX);
    double a1 = angle - M_PI / 6.0;
    double a2 = angle + M_PI / 6.0;

    QPointF tip(d.endX, d.endY);
    QPointF p1(d.endX - headLen * std::cos(a1), d.endY - headLen * std::sin(a1));
    QPointF p2(d.endX - headLen * std::cos(a2), d.endY - headLen * std::sin(a2));

    QPainterPath path;
    path.moveTo(tip);
    path.lineTo(p1);
    path.lineTo(p2);
    path.closeSubpath();

    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawPath(path);
    p.setBrush(Qt::NoBrush);
}

static void renderRect(QPainter &p, const RectData &d, const QColor &color, int sw) {
    QPen pen(color, sw, Qt::SolidLine, Qt::SquareCap, Qt::MiterJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawRect(QRectF(d.x, d.y, d.width, d.height));
}

static void renderCircle(QPainter &p, const CircleData &d, const QColor &color, int sw) {
    QPen pen(color, sw, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawEllipse(QPointF(d.cx, d.cy), d.rx, d.ry);
}

static void renderLine(QPainter &p, const LineData &d, const QColor &color, int sw) {
    QPen pen(color, sw, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));
}

static void renderDottedLine(QPainter &p, const DottedLineData &d, const QColor &color, int sw) {
    QPen pen(color, sw, Qt::CustomDashLine, Qt::RoundCap, Qt::RoundJoin);
    pen.setDashPattern({static_cast<qreal>(sw * 3), static_cast<qreal>(sw * 3)});
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));
}

static void renderFreehand(QPainter &p, const FreehandData &d, const QColor &color, int sw) {
    if (d.points.size() < 2) return;
    QPen pen(color, sw, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);

    QPainterPath path;
    path.moveTo(d.points[0]);
    for (int i = 1; i < d.points.size(); ++i) {
        path.lineTo(d.points[i]);
    }
    p.drawPath(path);
}

static void renderText(QPainter &p, const TextData &d, const QColor &color, int) {
    if (d.text.isEmpty()) return;
    QFont font("system-ui", d.fontSize);
    p.setFont(font);
    p.setPen(color);
    p.drawText(QPointF(d.x, d.y + d.fontSize), d.text);
}

static void renderHighlight(QPainter &p, const HighlightData &d, const QColor &color, int) {
    p.save();
    p.setOpacity(0.3);
    p.fillRect(QRectF(d.x, d.y, d.width, d.height), color);
    p.restore();
}

static void renderBlur(QPainter &p, const BlurData &d, const QColor &, int, const QImage &screenshot) {
    int blockSize = std::max(2, d.intensity);
    int imgX = static_cast<int>(d.x);
    int imgY = static_cast<int>(d.y);
    int imgW = static_cast<int>(d.width);
    int imgH = static_cast<int>(d.height);

    if (screenshot.isNull()) {
        // Fallback: dashed rect
        QPen pen(QColor(128, 128, 128), 1, Qt::DashLine);
        p.setPen(pen);
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(d.x, d.y, d.width, d.height));
        return;
    }

    for (int by = 0; by < imgH; by += blockSize) {
        for (int bx = 0; bx < imgW; bx += blockSize) {
            int cx = imgX + bx + blockSize / 2;
            int cy = imgY + by + blockSize / 2;
            cx = std::clamp(cx, 0, screenshot.width() - 1);
            cy = std::clamp(cy, 0, screenshot.height() - 1);
            QColor pixel = screenshot.pixelColor(cx, cy);

            int fillW = std::min(blockSize, imgW - bx);
            int fillH = std::min(blockSize, imgH - by);
            p.fillRect(QRectF(d.x + bx, d.y + by, fillW, fillH), pixel);
        }
    }
}

static void renderSteps(QPainter &p, const StepsData &d, const QColor &color, int sw) {
    double radius = 8.0 + sw * 3.0;

    // Filled circle
    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawEllipse(QPointF(d.x, d.y), radius, radius);

    // White number centered
    QFont font("system-ui", static_cast<int>(radius));
    font.setBold(true);
    p.setFont(font);
    p.setPen(Qt::white);
    QFontMetrics fm(font);
    QString numStr = QString::number(d.number);
    QRect textBounds = fm.boundingRect(numStr);
    p.drawText(QPointF(d.x - textBounds.width() / 2.0, d.y + textBounds.height() / 2.0 - fm.descent()), numStr);

    // Label box (if label is non-empty)
    if (!d.label.isEmpty()) {
        QFont labelFont("system-ui", static_cast<int>(radius - 2));
        labelFont.setBold(true);
        p.setFont(labelFont);
        QFontMetrics lfm(labelFont);
        QRect labelBounds = lfm.boundingRect(d.label);
        double lx = d.x + radius + 6;
        double ly = d.y - radius;
        double lw = labelBounds.width() + 16;
        double lh = radius * 2;

        p.setPen(Qt::NoPen);
        p.setBrush(color);
        p.drawRoundedRect(QRectF(lx, ly, lw, lh), 4, 4);

        p.setPen(Qt::white);
        p.drawText(QPointF(lx + 8, d.y + labelBounds.height() / 2.0 - lfm.descent()), d.label);
    }

    p.setBrush(Qt::NoBrush);
}

static void renderMeasure(QPainter &p, const MeasureData &d, const QColor &color, int sw) {
    // Dashed line
    QPen pen(color, sw, Qt::CustomDashLine, Qt::RoundCap);
    pen.setDashPattern({6.0, 4.0});
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));

    // Endpoint dots (3px filled circles)
    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawEllipse(QPointF(d.startX, d.startY), 3, 3);
    p.drawEllipse(QPointF(d.endX, d.endY), 3, 3);

    // Distance label at midpoint
    double dx = d.endX - d.startX;
    double dy = d.endY - d.startY;
    double dist = std::sqrt(dx * dx + dy * dy);
    QString label = QString("%1px").arg(static_cast<int>(dist));

    double mx = (d.startX + d.endX) / 2.0;
    double my = (d.startY + d.endY) / 2.0;

    QFont font("Menlo", 12);
    p.setFont(font);
    QFontMetrics fm(font);
    QRect labelRect = fm.boundingRect(label);

    double lx = mx - labelRect.width() / 2.0 - 4;
    double ly = my - labelRect.height() / 2.0 - 2;
    p.setPen(Qt::NoPen);
    p.setBrush(QColor(0, 0, 0, 180));
    p.drawRoundedRect(QRectF(lx, ly, labelRect.width() + 8, labelRect.height() + 4), 3, 3);

    p.setPen(Qt::white);
    p.drawText(QPointF(lx + 4, ly + labelRect.height() - fm.descent() + 1), label);

    p.setBrush(Qt::NoBrush);
}

void render(QPainter &p, const Annotation &a, const QImage &screenshotRegion) {
    std::visit([&](const auto &d) {
        using T = std::decay_t<decltype(d)>;
        if constexpr (std::is_same_v<T, ArrowData>) renderArrow(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, RectData>) renderRect(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, CircleData>) renderCircle(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, LineData>) renderLine(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, DottedLineData>) renderDottedLine(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, FreehandData>) renderFreehand(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, TextData>) renderText(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, HighlightData>) renderHighlight(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, BlurData>) renderBlur(p, d, a.color, a.strokeWidth, screenshotRegion);
        else if constexpr (std::is_same_v<T, StepsData>) renderSteps(p, d, a.color, a.strokeWidth);
        else if constexpr (std::is_same_v<T, MeasureData>) renderMeasure(p, d, a.color, a.strokeWidth);
        // ColorPickerData: no render
    }, a.data);
}

} // namespace AnnotationRenderer
```

- [ ] **Step 3: Add AnnotationRenderer.cpp to CMakeLists.txt**

Update `add_executable` in `qt/CMakeLists.txt`:

```cmake
add_executable(snapforge-qt
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
    src/AnnotationRenderer.cpp
)
```

- [ ] **Step 4: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add qt/src/AnnotationRenderer.h qt/src/AnnotationRenderer.cpp qt/CMakeLists.txt
git commit -m "feat(qt): add AnnotationRenderer with all 12 tool renderers"
```

---

### Task 4: Annotation canvas widget

**Files:**
- Create: `qt/src/AnnotationCanvas.h`
- Create: `qt/src/AnnotationCanvas.cpp`
- Modify: `qt/CMakeLists.txt`

- [ ] **Step 1: Create AnnotationCanvas header**

Create `qt/src/AnnotationCanvas.h`:

```cpp
#ifndef ANNOTATIONCANVAS_H
#define ANNOTATIONCANVAS_H

#include <QWidget>
#include <QLineEdit>
#include "AnnotationState.h"

class AnnotationCanvas : public QWidget {
    Q_OBJECT

public:
    explicit AnnotationCanvas(AnnotationState *state, const QImage &screenshot, QWidget *parent = nullptr);

    // Set the region this canvas covers (in parent widget coords)
    void setRegion(int x, int y, int w, int h);

    // Composite all annotations onto the screenshot region and return the result
    QImage compositeImage() const;

signals:
    void saved();
    void copied();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;

private:
    AnnotationState *m_state;
    QImage m_screenshot; // full pre-captured screenshot
    QImage m_regionScreenshot; // cropped to region for blur sampling

    int m_regionX = 0, m_regionY = 0;

    // Text input overlay
    QLineEdit *m_textInput = nullptr;
    bool m_waitingForText = false;

    void showTextInput(double x, double y);
    void commitTextInput();
    void cancelTextInput();

    // Drag tool helpers
    bool isDragTool(ToolType t) const;
    Annotation createDragAnnotation(ToolType tool, QPointF pos) const;
    void updateDragAnnotation(Annotation &a, QPointF pos) const;
    bool validateAnnotation(const Annotation &a) const;
    void normalizeAnnotation(Annotation &a) const;
};

#endif // ANNOTATIONCANVAS_H
```

- [ ] **Step 2: Create AnnotationCanvas implementation**

Create `qt/src/AnnotationCanvas.cpp`:

```cpp
#include "AnnotationCanvas.h"
#include "AnnotationRenderer.h"
#include <QPainter>
#include <QMouseEvent>
#include <cmath>

AnnotationCanvas::AnnotationCanvas(AnnotationState *state, const QImage &screenshot, QWidget *parent)
    : QWidget(parent), m_state(state), m_screenshot(screenshot)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setMouseTracking(true);
    setCursor(Qt::CrossCursor);

    connect(m_state, &AnnotationState::changed, this, [this]() { update(); });
}

void AnnotationCanvas::setRegion(int x, int y, int w, int h) {
    m_regionX = x;
    m_regionY = y;
    setGeometry(x, y, w, h);

    // Crop screenshot to this region for blur pixel sampling
    if (!m_screenshot.isNull()) {
        double dpr = m_screenshot.width() / static_cast<double>(parentWidget()->width());
        int px = static_cast<int>(x * dpr);
        int py = static_cast<int>(y * dpr);
        int pw = static_cast<int>(w * dpr);
        int ph = static_cast<int>(h * dpr);
        px = std::clamp(px, 0, m_screenshot.width() - 1);
        py = std::clamp(py, 0, m_screenshot.height() - 1);
        pw = std::min(pw, m_screenshot.width() - px);
        ph = std::min(ph, m_screenshot.height() - py);
        m_regionScreenshot = m_screenshot.copy(px, py, pw, ph);
    }
}

QImage AnnotationCanvas::compositeImage() const {
    if (m_regionScreenshot.isNull()) return QImage();

    QImage result = m_regionScreenshot.copy();
    QPainter p(&result);
    p.setRenderHint(QPainter::Antialiasing);

    // Scale painter so annotation coords (in widget/point space) map to pixel space
    double dpr = result.width() / static_cast<double>(width());
    p.scale(dpr, dpr);

    for (const auto &a : m_state->annotations()) {
        AnnotationRenderer::render(p, a, m_regionScreenshot);
    }
    p.end();
    return result;
}

void AnnotationCanvas::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    // Render committed annotations
    for (const auto &a : m_state->annotations()) {
        AnnotationRenderer::render(p, a, m_regionScreenshot);
    }

    // Render active annotation (except text/steps — those use input overlay)
    const auto &active = m_state->activeAnnotation();
    if (active && active->tool != ToolType::Text && active->tool != ToolType::Steps) {
        AnnotationRenderer::render(p, *active, m_regionScreenshot);
    }
}

bool AnnotationCanvas::isDragTool(ToolType t) const {
    return t == ToolType::Arrow || t == ToolType::Rect || t == ToolType::Circle ||
           t == ToolType::Line || t == ToolType::DottedLine || t == ToolType::Freehand ||
           t == ToolType::Highlight || t == ToolType::Blur || t == ToolType::Measure;
}

Annotation AnnotationCanvas::createDragAnnotation(ToolType tool, QPointF pos) const {
    Annotation a;
    a.id = AnnotationState::newId();
    a.tool = tool;
    a.color = m_state->strokeColor();
    a.strokeWidth = m_state->strokeWidth();

    switch (tool) {
    case ToolType::Arrow:
        a.data = ArrowData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::Rect:
        a.data = RectData{pos.x(), pos.y(), 0, 0};
        break;
    case ToolType::Circle:
        a.data = CircleData{pos.x(), pos.y(), 0, 0};
        break;
    case ToolType::Line:
        a.data = LineData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::DottedLine:
        a.data = DottedLineData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    case ToolType::Freehand:
        a.data = FreehandData{{pos}};
        break;
    case ToolType::Highlight:
        a.data = HighlightData{pos.x(), pos.y(), 0, 0};
        break;
    case ToolType::Blur:
        a.data = BlurData{pos.x(), pos.y(), 0, 0, 10};
        break;
    case ToolType::Measure:
        a.data = MeasureData{pos.x(), pos.y(), pos.x(), pos.y()};
        break;
    default:
        break;
    }
    return a;
}

void AnnotationCanvas::updateDragAnnotation(Annotation &a, QPointF pos) const {
    std::visit([&pos, &a](auto &d) {
        using T = std::decay_t<decltype(d)>;
        if constexpr (std::is_same_v<T, ArrowData> || std::is_same_v<T, LineData> ||
                      std::is_same_v<T, DottedLineData> || std::is_same_v<T, MeasureData>) {
            d.endX = pos.x(); d.endY = pos.y();
        } else if constexpr (std::is_same_v<T, RectData> || std::is_same_v<T, HighlightData>) {
            d.width = pos.x() - d.x;
            d.height = pos.y() - d.y;
        } else if constexpr (std::is_same_v<T, BlurData>) {
            d.width = pos.x() - d.x;
            d.height = pos.y() - d.y;
        } else if constexpr (std::is_same_v<T, CircleData>) {
            d.rx = std::abs(pos.x() - d.cx);
            d.ry = std::abs(pos.y() - d.cy);
        } else if constexpr (std::is_same_v<T, FreehandData>) {
            d.points.push_back(pos);
        }
    }, a.data);
}

bool AnnotationCanvas::validateAnnotation(const Annotation &a) const {
    return std::visit([](const auto &d) -> bool {
        using T = std::decay_t<decltype(d)>;
        if constexpr (std::is_same_v<T, ArrowData> || std::is_same_v<T, LineData> ||
                      std::is_same_v<T, DottedLineData> || std::is_same_v<T, MeasureData>) {
            double dx = d.endX - d.startX;
            double dy = d.endY - d.startY;
            return std::sqrt(dx*dx + dy*dy) > 3.0;
        } else if constexpr (std::is_same_v<T, RectData> || std::is_same_v<T, HighlightData>) {
            return std::abs(d.width) > 3 && std::abs(d.height) > 3;
        } else if constexpr (std::is_same_v<T, BlurData>) {
            return std::abs(d.width) > 3 && std::abs(d.height) > 3;
        } else if constexpr (std::is_same_v<T, CircleData>) {
            return d.rx > 3 && d.ry > 3;
        } else if constexpr (std::is_same_v<T, FreehandData>) {
            return d.points.size() > 2;
        } else {
            return true;
        }
    }, a.data);
}

void AnnotationCanvas::normalizeAnnotation(Annotation &a) const {
    std::visit([](auto &d) {
        using T = std::decay_t<decltype(d)>;
        if constexpr (std::is_same_v<T, RectData> || std::is_same_v<T, HighlightData>) {
            if (d.width < 0) { d.x += d.width; d.width = -d.width; }
            if (d.height < 0) { d.y += d.height; d.height = -d.height; }
        } else if constexpr (std::is_same_v<T, BlurData>) {
            if (d.width < 0) { d.x += d.width; d.width = -d.width; }
            if (d.height < 0) { d.y += d.height; d.height = -d.height; }
        }
    }, a.data);
}

void AnnotationCanvas::mousePressEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) return;
    QPointF pos = event->position();
    ToolType tool = m_state->activeTool();

    // Cancel any pending text input
    if (m_waitingForText) {
        cancelTextInput();
    }

    if (tool == ToolType::ColorPicker) {
        // Sample pixel from screenshot
        if (!m_regionScreenshot.isNull()) {
            double dpr = m_regionScreenshot.width() / static_cast<double>(width());
            int px = static_cast<int>(pos.x() * dpr);
            int py = static_cast<int>(pos.y() * dpr);
            px = std::clamp(px, 0, m_regionScreenshot.width() - 1);
            py = std::clamp(py, 0, m_regionScreenshot.height() - 1);
            QColor sampled = m_regionScreenshot.pixelColor(px, py);
            m_state->setStrokeColor(sampled);
        }
        return;
    }

    if (tool == ToolType::Text) {
        showTextInput(pos.x(), pos.y());
        return;
    }

    if (tool == ToolType::Steps) {
        // Create steps annotation, show label input
        Annotation a;
        a.id = AnnotationState::newId();
        a.tool = ToolType::Steps;
        a.color = m_state->strokeColor();
        a.strokeWidth = m_state->strokeWidth();
        a.data = StepsData{pos.x(), pos.y(), m_state->nextStepNumber(), ""};
        m_state->setActiveAnnotation(a);
        m_state->incrementStepNumber();
        showTextInput(pos.x() + 28, pos.y() - 12);
        return;
    }

    if (isDragTool(tool)) {
        Annotation a = createDragAnnotation(tool, pos);
        m_state->setActiveAnnotation(a);
    }
}

void AnnotationCanvas::mouseMoveEvent(QMouseEvent *event) {
    auto active = m_state->activeAnnotation();
    if (!active || m_waitingForText) return;

    Annotation a = *active;
    updateDragAnnotation(a, event->position());
    m_state->setActiveAnnotation(a);
}

void AnnotationCanvas::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) return;
    auto active = m_state->activeAnnotation();
    if (!active || m_waitingForText) return;

    Annotation a = *active;
    if (validateAnnotation(a)) {
        normalizeAnnotation(a);
        m_state->commitAnnotation(a);
    } else {
        m_state->clearActiveAnnotation();
    }
}

void AnnotationCanvas::showTextInput(double x, double y) {
    if (!m_textInput) {
        m_textInput = new QLineEdit(this);
        m_textInput->setStyleSheet(
            "QLineEdit {"
            "  background: rgba(0, 0, 0, 0.6);"
            "  color: white;"
            "  border: 1px dashed rgba(255, 255, 255, 0.6);"
            "  padding: 4px 8px;"
            "  font-size: 14px;"
            "  font-family: system-ui;"
            "}"
        );
        connect(m_textInput, &QLineEdit::returnPressed, this, &AnnotationCanvas::commitTextInput);
    }

    m_textInput->clear();
    m_textInput->setGeometry(static_cast<int>(x), static_cast<int>(y), 200, 30);
    m_textInput->show();
    m_textInput->setFocus();
    m_waitingForText = true;
}

void AnnotationCanvas::commitTextInput() {
    if (!m_textInput || !m_waitingForText) return;
    QString text = m_textInput->text();
    m_textInput->hide();
    m_waitingForText = false;

    auto active = m_state->activeAnnotation();
    if (active) {
        Annotation a = *active;
        if (a.tool == ToolType::Text) {
            auto &td = std::get<TextData>(a.data);
            td.text = text;
            if (!text.isEmpty()) {
                m_state->commitAnnotation(a);
            } else {
                m_state->clearActiveAnnotation();
            }
        } else if (a.tool == ToolType::Steps) {
            auto &sd = std::get<StepsData>(a.data);
            sd.label = text;
            // Steps always commit (label can be empty)
            m_state->commitAnnotation(a);
        }
    } else if (!text.isEmpty()) {
        // Text tool without active annotation (shouldn't happen but handle gracefully)
        Annotation a;
        a.id = AnnotationState::newId();
        a.tool = ToolType::Text;
        a.color = m_state->strokeColor();
        a.strokeWidth = m_state->strokeWidth();
        // We lost the position — this is a fallback
        a.data = TextData{10, 20, text, 16};
        m_state->commitAnnotation(a);
    }
}

void AnnotationCanvas::cancelTextInput() {
    if (m_textInput) {
        m_textInput->hide();
    }
    m_waitingForText = false;
    m_state->clearActiveAnnotation();
}
```

- [ ] **Step 3: Add AnnotationCanvas.cpp to CMakeLists.txt**

Update `add_executable` in `qt/CMakeLists.txt`:

```cmake
add_executable(snapforge-qt
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
    src/AnnotationRenderer.cpp
    src/AnnotationCanvas.cpp
)
```

- [ ] **Step 4: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add qt/src/AnnotationCanvas.h qt/src/AnnotationCanvas.cpp qt/CMakeLists.txt
git commit -m "feat(qt): add AnnotationCanvas with mouse handling for all tools"
```

---

### Task 5: Annotation toolbar

**Files:**
- Create: `qt/src/AnnotationToolbar.h`
- Create: `qt/src/AnnotationToolbar.cpp`
- Modify: `qt/CMakeLists.txt`

- [ ] **Step 1: Create AnnotationToolbar header**

Create `qt/src/AnnotationToolbar.h`:

```cpp
#ifndef ANNOTATIONTOOLBAR_H
#define ANNOTATIONTOOLBAR_H

#include <QWidget>
#include <QPushButton>
#include <QMap>
#include "AnnotationState.h"

class AnnotationToolbar : public QWidget {
    Q_OBJECT

public:
    explicit AnnotationToolbar(AnnotationState *state, QWidget *parent = nullptr);

    // Position the toolbar relative to the selected region
    void positionRelativeTo(int regionX, int regionY, int regionW, int regionH);

signals:
    void saveRequested();
    void copyRequested();
    void cancelRequested();

private:
    AnnotationState *m_state;
    QMap<ToolType, QPushButton*> m_toolButtons;
    QVector<QPushButton*> m_colorButtons;
    QVector<QPushButton*> m_sizeButtons;
    QPushButton *m_undoBtn = nullptr;
    QPushButton *m_redoBtn = nullptr;

    // Dragging
    bool m_dragging = false;
    QPoint m_dragOffset;

    void setupUI();
    void updateToolHighlight();
    void updateUndoRedoState();

protected:
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
};

#endif // ANNOTATIONTOOLBAR_H
```

- [ ] **Step 2: Create AnnotationToolbar implementation**

Create `qt/src/AnnotationToolbar.cpp`:

```cpp
#include "AnnotationToolbar.h"
#include <QHBoxLayout>
#include <QMouseEvent>
#include <QColorDialog>
#include <QFrame>
#include <QScreen>
#include <QGuiApplication>

static const char *toolLabels[] = {
    "↗", "□", "○", "─", "┄", "✎", "T", "▬", "▦", "①", "◉", "↔"
};

static const ToolType toolTypes[] = {
    ToolType::Arrow, ToolType::Rect, ToolType::Circle, ToolType::Line,
    ToolType::DottedLine, ToolType::Freehand, ToolType::Text, ToolType::Highlight,
    ToolType::Blur, ToolType::Steps, ToolType::ColorPicker, ToolType::Measure
};

static const char *presetColors[] = {
    "#FF0000", "#00FF00", "#0000FF", "#FFFF00",
    "#FF00FF", "#00FFFF", "#FFFFFF", "#000000"
};

AnnotationToolbar::AnnotationToolbar(AnnotationState *state, QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool),
      m_state(state)
{
    setAttribute(Qt::WA_TranslucentBackground, false);
    setStyleSheet(
        "AnnotationToolbar {"
        "  background: rgba(30, 30, 30, 0.92);"
        "  border-radius: 8px;"
        "  padding: 4px;"
        "}"
    );
    setCursor(Qt::ArrowCursor);
    setupUI();

    connect(m_state, &AnnotationState::toolChanged, this, [this](ToolType) { updateToolHighlight(); });
    connect(m_state, &AnnotationState::changed, this, [this]() { updateUndoRedoState(); });
}

void AnnotationToolbar::setupUI() {
    auto *layout = new QHBoxLayout(this);
    layout->setContentsMargins(6, 4, 6, 4);
    layout->setSpacing(2);

    QString btnStyle =
        "QPushButton {"
        "  background: transparent;"
        "  color: rgba(255,255,255,0.8);"
        "  border: none;"
        "  border-radius: 4px;"
        "  padding: 4px 6px;"
        "  font-size: 14px;"
        "  min-width: 24px;"
        "  min-height: 24px;"
        "}"
        "QPushButton:hover {"
        "  background: rgba(255,255,255,0.15);"
        "}"
        "QPushButton:checked, QPushButton[active=\"true\"] {"
        "  background: rgba(255,255,255,0.25);"
        "  color: white;"
        "}";

    // Tool buttons
    for (int i = 0; i < 12; ++i) {
        auto *btn = new QPushButton(toolLabels[i], this);
        btn->setStyleSheet(btnStyle);
        btn->setCheckable(true);
        ToolType tt = toolTypes[i];
        connect(btn, &QPushButton::clicked, this, [this, tt]() {
            m_state->setActiveTool(tt);
        });
        layout->addWidget(btn);
        m_toolButtons[tt] = btn;
    }

    // Separator
    auto *sep1 = new QFrame(this);
    sep1->setFrameShape(QFrame::VLine);
    sep1->setStyleSheet("QFrame { color: rgba(255,255,255,0.2); }");
    layout->addWidget(sep1);

    // Color swatches
    for (int i = 0; i < 8; ++i) {
        auto *btn = new QPushButton(this);
        btn->setFixedSize(18, 18);
        QColor c(presetColors[i]);
        btn->setStyleSheet(QString(
            "QPushButton {"
            "  background: %1;"
            "  border: 1px solid rgba(255,255,255,0.3);"
            "  border-radius: 3px;"
            "  min-width: 18px; min-height: 18px;"
            "  max-width: 18px; max-height: 18px;"
            "}"
            "QPushButton:hover { border-color: white; }"
        ).arg(c.name()));
        connect(btn, &QPushButton::clicked, this, [this, c]() {
            m_state->setStrokeColor(c);
        });
        layout->addWidget(btn);
        m_colorButtons.push_back(btn);
    }

    // Custom color button
    auto *customColorBtn = new QPushButton("+", this);
    customColorBtn->setFixedSize(18, 18);
    customColorBtn->setStyleSheet(
        "QPushButton { background: transparent; color: white; border: 1px dashed rgba(255,255,255,0.4); border-radius: 3px; font-size: 12px; min-width: 18px; min-height: 18px; max-width: 18px; max-height: 18px; }"
        "QPushButton:hover { border-color: white; }"
    );
    connect(customColorBtn, &QPushButton::clicked, this, [this]() {
        QColor c = QColorDialog::getColor(m_state->strokeColor(), this);
        if (c.isValid()) m_state->setStrokeColor(c);
    });
    layout->addWidget(customColorBtn);

    // Separator
    auto *sep2 = new QFrame(this);
    sep2->setFrameShape(QFrame::VLine);
    sep2->setStyleSheet("QFrame { color: rgba(255,255,255,0.2); }");
    layout->addWidget(sep2);

    // Stroke width buttons
    int sizes[] = {1, 2, 4};
    int dotSizes[] = {4, 7, 10};
    for (int i = 0; i < 3; ++i) {
        auto *btn = new QPushButton(this);
        btn->setFixedSize(20, 20);
        int ds = dotSizes[i];
        int sz = sizes[i];
        btn->setStyleSheet(QString(
            "QPushButton {"
            "  background: transparent; border: none; border-radius: 4px;"
            "  min-width: 20px; min-height: 20px; max-width: 20px; max-height: 20px;"
            "}"
            "QPushButton:hover { background: rgba(255,255,255,0.15); }"
        ));
        // Draw dot via icon (or we just use text with a bullet)
        btn->setText(QString("●").repeated(1));
        QFont f = btn->font();
        f.setPointSize(ds);
        btn->setFont(f);
        btn->setStyleSheet(btn->styleSheet() + "QPushButton { color: rgba(255,255,255,0.8); }");
        connect(btn, &QPushButton::clicked, this, [this, sz]() {
            m_state->setStrokeWidth(sz);
        });
        layout->addWidget(btn);
        m_sizeButtons.push_back(btn);
    }

    // Separator
    auto *sep3 = new QFrame(this);
    sep3->setFrameShape(QFrame::VLine);
    sep3->setStyleSheet("QFrame { color: rgba(255,255,255,0.2); }");
    layout->addWidget(sep3);

    // Undo / Redo
    m_undoBtn = new QPushButton("↶", this);
    m_undoBtn->setStyleSheet(btnStyle);
    connect(m_undoBtn, &QPushButton::clicked, m_state, &AnnotationState::undo);
    layout->addWidget(m_undoBtn);

    m_redoBtn = new QPushButton("↷", this);
    m_redoBtn->setStyleSheet(btnStyle);
    connect(m_redoBtn, &QPushButton::clicked, m_state, &AnnotationState::redo);
    layout->addWidget(m_redoBtn);

    // Separator
    auto *sep4 = new QFrame(this);
    sep4->setFrameShape(QFrame::VLine);
    sep4->setStyleSheet("QFrame { color: rgba(255,255,255,0.2); }");
    layout->addWidget(sep4);

    // Save / Copy / Cancel
    auto *saveBtn = new QPushButton("Save", this);
    saveBtn->setStyleSheet(btnStyle + "QPushButton { color: #4CAF50; font-weight: bold; }");
    connect(saveBtn, &QPushButton::clicked, this, &AnnotationToolbar::saveRequested);
    layout->addWidget(saveBtn);

    auto *copyBtn = new QPushButton("Copy", this);
    copyBtn->setStyleSheet(btnStyle + "QPushButton { color: #2196F3; font-weight: bold; }");
    connect(copyBtn, &QPushButton::clicked, this, &AnnotationToolbar::copyRequested);
    layout->addWidget(copyBtn);

    auto *cancelBtn = new QPushButton("Cancel", this);
    cancelBtn->setStyleSheet(btnStyle + "QPushButton { color: rgba(255,255,255,0.5); }");
    connect(cancelBtn, &QPushButton::clicked, this, &AnnotationToolbar::cancelRequested);
    layout->addWidget(cancelBtn);

    updateToolHighlight();
    updateUndoRedoState();
}

void AnnotationToolbar::updateToolHighlight() {
    ToolType active = m_state->activeTool();
    for (auto it = m_toolButtons.begin(); it != m_toolButtons.end(); ++it) {
        it.value()->setChecked(it.key() == active);
    }
}

void AnnotationToolbar::updateUndoRedoState() {
    if (m_undoBtn) m_undoBtn->setEnabled(m_state->canUndo());
    if (m_redoBtn) m_redoBtn->setEnabled(m_state->canRedo());
}

void AnnotationToolbar::positionRelativeTo(int regionX, int regionY, int regionW, int regionH) {
    adjustSize();
    int tw = sizeHint().width();
    int th = sizeHint().height();

    // Center below region
    int x = regionX + regionW / 2 - tw / 2;
    int y = regionY + regionH + 12;

    // If toolbar goes off screen bottom, put it above the region
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        int screenH = screen->geometry().height();
        if (y + th > screenH) {
            y = regionY - th - 12;
        }
    }

    move(x, y);
}

void AnnotationToolbar::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        // Only drag from non-button areas (check if child at pos is null)
        QWidget *child = childAt(event->pos());
        if (!child || child == this) {
            m_dragging = true;
            m_dragOffset = event->pos();
        }
    }
    QWidget::mousePressEvent(event);
}

void AnnotationToolbar::mouseMoveEvent(QMouseEvent *event) {
    if (m_dragging) {
        move(mapToParent(event->pos() - m_dragOffset));
    }
    QWidget::mouseMoveEvent(event);
}

void AnnotationToolbar::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        m_dragging = false;
    }
    QWidget::mouseReleaseEvent(event);
}
```

- [ ] **Step 3: Add AnnotationToolbar.cpp to CMakeLists.txt**

Update `add_executable` in `qt/CMakeLists.txt`:

```cmake
add_executable(snapforge-qt
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
    src/AnnotationRenderer.cpp
    src/AnnotationCanvas.cpp
    src/AnnotationToolbar.cpp
)
```

- [ ] **Step 4: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add qt/src/AnnotationToolbar.h qt/src/AnnotationToolbar.cpp qt/CMakeLists.txt
git commit -m "feat(qt): add AnnotationToolbar with tools, colors, sizes, and actions"
```

---

### Task 6: Integrate annotation into OverlayWindow

**Files:**
- Modify: `qt/src/OverlayWindow.h`
- Modify: `qt/src/OverlayWindow.cpp`

- [ ] **Step 1: Update OverlayWindow header with annotate mode**

Replace `qt/src/OverlayWindow.h` with:

```cpp
#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>
#include <QThread>
#include "AnnotationState.h"

class AnnotationCanvas;
class AnnotationToolbar;

class CaptureWorker : public QThread {
    Q_OBJECT
public:
    void run() override;
signals:
    void captured(QImage image);
};

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();

signals:
    void screenshotReady(QImage composited, int w, int h);
    void clipboardReady(QImage composited, int w, int h);
    void cancelled();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    enum class Mode { Select, Annotate };
    Mode m_mode = Mode::Select;

    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    QImage m_screenshot;
    CaptureWorker m_captureWorker;

    // Annotation
    AnnotationState m_annotationState;
    AnnotationCanvas *m_canvas = nullptr;
    AnnotationToolbar *m_toolbar = nullptr;

    QRect selectedRect() const;
    void enterAnnotateMode();
    void exitAnnotateMode();
    void handleSave();
    void handleCopy();
    bool isOnRegionEdge(QPoint pos) const;

    // Tool keyboard shortcuts
    static const QMap<int, ToolType> &toolShortcuts();
};

#endif // OVERLAYWINDOW_H
```

- [ ] **Step 2: Update OverlayWindow implementation**

Replace `qt/src/OverlayWindow.cpp` with:

```cpp
#include "OverlayWindow.h"
#include "AnnotationCanvas.h"
#include "AnnotationToolbar.h"
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QScreen>
#include <QGuiApplication>
#include <QElapsedTimer>
#include <QFontMetrics>
#include "snapforge_ffi.h"

// --- CaptureWorker ---

void CaptureWorker::run() {
    CapturedImage img = snapforge_capture_fullscreen(0);
    if (img.data && img.width > 0) {
        QImage qimg(img.data, img.width, img.height, img.width * 4,
                    QImage::Format_RGBA8888);
        QImage copy = qimg.copy();
        snapforge_free_buffer(img.data, img.len);
        emit captured(copy);
    }
}

// --- OverlayWindow ---

const QMap<int, ToolType> &OverlayWindow::toolShortcuts() {
    static const QMap<int, ToolType> map = {
        {Qt::Key_A, ToolType::Arrow},
        {Qt::Key_R, ToolType::Rect},
        {Qt::Key_C, ToolType::Circle},
        {Qt::Key_L, ToolType::Line},
        {Qt::Key_D, ToolType::DottedLine},
        {Qt::Key_F, ToolType::Freehand},
        {Qt::Key_T, ToolType::Text},
        {Qt::Key_H, ToolType::Highlight},
        {Qt::Key_B, ToolType::Blur},
        {Qt::Key_N, ToolType::Steps},
        {Qt::Key_I, ToolType::ColorPicker},
        {Qt::Key_M, ToolType::Measure},
    };
    return map;
}

OverlayWindow::OverlayWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_ShowWithoutActivating, false);
    setCursor(Qt::CrossCursor);
    setMouseTracking(true);

    QFont font("Menlo", 11);
    QFontMetrics fm(font);
    fm.boundingRect("0");

    connect(&m_captureWorker, &CaptureWorker::captured, this, [this](QImage image) {
        m_screenshot = image;
        update();
    });
}

void OverlayWindow::activate() {
    QElapsedTimer timer;
    timer.start();

    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        setGeometry(screen->geometry());
    }

    m_mode = Mode::Select;
    m_drawing = false;
    m_hasRegion = false;
    m_startPos = QPoint();
    m_endPos = QPoint();
    m_screenshot = QImage();
    m_annotationState.clearAnnotations();
    exitAnnotateMode();

    showFullScreen();
    activateWindow();
    raise();

    qDebug("Overlay shown in %lld ms", timer.elapsed());

    if (!m_captureWorker.isRunning()) {
        m_captureWorker.start();
    }
}

QRect OverlayWindow::selectedRect() const {
    return QRect(m_startPos, m_endPos).normalized();
}

bool OverlayWindow::isOnRegionEdge(QPoint pos) const {
    if (!m_hasRegion) return false;
    QRect sel = selectedRect();
    if (!sel.contains(pos)) return false;
    int margin = 6;
    return (pos.x() - sel.x() < margin || sel.right() - pos.x() < margin ||
            pos.y() - sel.y() < margin || sel.bottom() - pos.y() < margin);
}

void OverlayWindow::enterAnnotateMode() {
    m_mode = Mode::Annotate;
    QRect sel = selectedRect();

    if (!m_canvas) {
        m_canvas = new AnnotationCanvas(&m_annotationState, m_screenshot, this);
    }
    m_canvas->setRegion(sel.x(), sel.y(), sel.width(), sel.height());
    m_canvas->show();
    m_canvas->raise();

    if (!m_toolbar) {
        m_toolbar = new AnnotationToolbar(&m_annotationState, this);
        connect(m_toolbar, &AnnotationToolbar::saveRequested, this, &OverlayWindow::handleSave);
        connect(m_toolbar, &AnnotationToolbar::copyRequested, this, &OverlayWindow::handleCopy);
        connect(m_toolbar, &AnnotationToolbar::cancelRequested, this, [this]() {
            exitAnnotateMode();
            m_hasRegion = false;
            m_drawing = false;
            hide();
            emit cancelled();
        });
    }
    m_toolbar->positionRelativeTo(sel.x(), sel.y(), sel.width(), sel.height());
    m_toolbar->show();
    m_toolbar->raise();

    update();
}

void OverlayWindow::exitAnnotateMode() {
    if (m_canvas) { m_canvas->hide(); }
    if (m_toolbar) { m_toolbar->hide(); }
    m_mode = Mode::Select;
}

void OverlayWindow::handleSave() {
    if (!m_canvas) return;
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) return;

    hide();
    exitAnnotateMode();
    emit screenshotReady(composited, composited.width(), composited.height());
}

void OverlayWindow::handleCopy() {
    if (!m_canvas) return;
    QImage composited = m_canvas->compositeImage();
    if (composited.isNull()) return;

    hide();
    exitAnnotateMode();
    emit clipboardReady(composited, composited.width(), composited.height());
}

void OverlayWindow::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    if (!m_screenshot.isNull()) {
        p.drawImage(0, 0, m_screenshot.scaled(size(), Qt::IgnoreAspectRatio, Qt::FastTransformation));
    }

    if (m_drawing || m_hasRegion) {
        QRect sel = selectedRect();

        QPainterPath fullPath;
        fullPath.addRect(rect());
        QPainterPath selPath;
        selPath.addRect(sel);
        QPainterPath dimPath = fullPath.subtracted(selPath);
        p.fillPath(dimPath, QColor(0, 0, 0, 100));

        QPen whitePen(Qt::white, 1, Qt::DashLine);
        p.setPen(whitePen);
        p.drawRect(sel);

        QPen darkPen(QColor(0, 0, 0, 150), 1, Qt::DashLine);
        darkPen.setDashOffset(4);
        p.setPen(darkPen);
        p.drawRect(sel);

        // Dimension label
        QString label = QString("%1 × %2").arg(sel.width()).arg(sel.height());
        QFont font("Menlo", 11);
        p.setFont(font);
        QFontMetrics fm(font);
        QRect labelRect = fm.boundingRect(label);
        int lx = sel.x() + sel.width() / 2 - labelRect.width() / 2 - 4;
        int ly = sel.y() - labelRect.height() - 8;
        p.fillRect(lx - 2, ly - 1, labelRect.width() + 8, labelRect.height() + 4,
                   QColor(0, 0, 0, 180));
        p.setPen(Qt::white);
        p.drawText(lx + 2, ly + labelRect.height() - 2, label);

        // Resize handles
        if (m_hasRegion && !m_drawing && m_mode == Mode::Select) {
            p.setPen(QColor(0, 0, 0, 128));
            p.setBrush(Qt::white);
            int hs = 4;
            QPoint handles[] = {
                sel.topLeft(), sel.topRight(), sel.bottomLeft(), sel.bottomRight(),
                QPoint(sel.center().x(), sel.top()),
                QPoint(sel.center().x(), sel.bottom()),
                QPoint(sel.left(), sel.center().y()),
                QPoint(sel.right(), sel.center().y()),
            };
            for (const auto &pt : handles) {
                p.drawRect(pt.x() - hs, pt.y() - hs, hs * 2, hs * 2);
            }
        }
    } else {
        p.fillRect(rect(), QColor(0, 0, 0, 40));
    }
}

void OverlayWindow::mousePressEvent(QMouseEvent *event) {
    if (event->button() != Qt::LeftButton) return;

    if (m_mode == Mode::Annotate) {
        QRect sel = selectedRect();
        // Click outside region → start new selection
        if (!sel.contains(event->pos()) && !isOnRegionEdge(event->pos())) {
            exitAnnotateMode();
            m_annotationState.clearAnnotations();
            m_hasRegion = false;
            m_startPos = event->pos();
            m_endPos = event->pos();
            m_drawing = true;
            update();
        }
        // Inside region: AnnotationCanvas handles it
        return;
    }

    // Select mode
    m_startPos = event->pos();
    m_endPos = event->pos();
    m_drawing = true;
    m_hasRegion = false;
    update();
}

void OverlayWindow::mouseMoveEvent(QMouseEvent *event) {
    if (m_drawing) {
        m_endPos = event->pos();
        update();
    }
}

void OverlayWindow::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton && m_drawing) {
        m_drawing = false;
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            m_hasRegion = true;
            enterAnnotateMode();
        }
        update();
    }
}

void OverlayWindow::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_Escape) {
        if (m_mode == Mode::Annotate) {
            exitAnnotateMode();
            m_annotationState.clearAnnotations();
            m_hasRegion = false;
        }
        m_drawing = false;
        hide();
        emit cancelled();
        return;
    }

    if (m_mode == Mode::Annotate) {
        // Undo/redo
        if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_Z) {
            if (event->modifiers() & Qt::ShiftModifier) {
                m_annotationState.redo();
            } else {
                m_annotationState.undo();
            }
            return;
        }
        // Save
        if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_S) {
            handleSave();
            return;
        }
        if (event->key() == Qt::Key_Return) {
            handleSave();
            return;
        }
        // Copy
        if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_C) {
            handleCopy();
            return;
        }

        // Tool shortcuts (only without modifiers)
        if (!(event->modifiers() & (Qt::ControlModifier | Qt::AltModifier | Qt::MetaModifier))) {
            // Stroke width shortcuts
            if (event->key() == Qt::Key_1) { m_annotationState.setStrokeWidth(1); return; }
            if (event->key() == Qt::Key_2) { m_annotationState.setStrokeWidth(2); return; }
            if (event->key() == Qt::Key_3) { m_annotationState.setStrokeWidth(4); return; }

            // Tool shortcuts
            auto it = toolShortcuts().find(event->key());
            if (it != toolShortcuts().end()) {
                m_annotationState.setActiveTool(it.value());
                return;
            }
        }
    }
}
```

- [ ] **Step 3: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add qt/src/OverlayWindow.h qt/src/OverlayWindow.cpp
git commit -m "feat(qt): integrate annotation canvas and toolbar into overlay"
```

---

### Task 7: Update main.cpp for composited save/copy

**Files:**
- Modify: `qt/src/main.cpp`

- [ ] **Step 1: Update main.cpp to handle composited images**

Replace `qt/src/main.cpp` with:

```cpp
#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QDir>
#include <QDateTime>
#include <QTimer>
#include "OverlayWindow.h"
#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>

static OverlayWindow *g_overlay = nullptr;

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef, void *) {
    if (g_overlay) {
        QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
    }
    return noErr;
}

void registerGlobalHotkey() {
    EventHotKeyRef hotKeyRef;
    EventHotKeyID hotKeyID;
    hotKeyID.signature = 'SNPF';
    hotKeyID.id = 1;

    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    RegisterEventHotKey(0x01, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &hotKeyRef);
}
#endif

static void saveImage(const QImage &img) {
    if (img.isNull()) return;

    // Convert QImage to RGBA bytes
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);

    char *saveDir = snapforge_default_save_path();
    QString dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Pictures/Snapforge";
    if (saveDir) snapforge_free_string(saveDir);

    QDir().mkpath(dir);
    QString timestamp = QDateTime::currentDateTime().toString("yyyy-MM-dd_HH-mm-ss");
    QString path = dir + "/screenshot_" + timestamp + ".png";

    int result = snapforge_save_image(rgba.constBits(), rgba.width(), rgba.height(),
                                      path.toUtf8().constData(), 0, 90);
    if (result == 0) {
        qDebug("Saved: %s", qPrintable(path));
    }
}

static void copyImage(const QImage &img) {
    if (img.isNull()) return;

    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);
    snapforge_copy_to_clipboard(rgba.constBits(), rgba.width(), rgba.height());
    qDebug("Copied to clipboard");
}

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
    app.setQuitOnLastWindowClosed(false);

#ifdef Q_OS_MAC
    app.setAttribute(Qt::AA_DontShowIconsInMenus);
#endif

    if (!snapforge_has_permission()) {
        snapforge_request_permission();
    }

    OverlayWindow overlay;
    g_overlay = &overlay;
    overlay.showFullScreen();
    overlay.hide();

    // Handle composited screenshot save
    QObject::connect(&overlay, &OverlayWindow::screenshotReady,
                     [](QImage composited, int, int) {
        saveImage(composited);
    });

    // Handle composited clipboard copy
    QObject::connect(&overlay, &OverlayWindow::clipboardReady,
                     [](QImage composited, int, int) {
        copyImage(composited);
    });

    // System tray
    QSystemTrayIcon tray;
    tray.setIcon(QIcon::fromTheme("camera-photo"));
    tray.setToolTip("Snapforge");

    QMenu menu;
    menu.addAction("Screenshot (Cmd+Shift+S)", &overlay, &OverlayWindow::activate);
    menu.addSeparator();
    menu.addAction("Quit", &app, &QApplication::quit);
    tray.setContextMenu(&menu);
    tray.show();

#ifdef Q_OS_MAC
    registerGlobalHotkey();
#endif

    return app.exec();
}
```

- [ ] **Step 2: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake -B build -DCMAKE_PREFIX_PATH=/opt/homebrew/opt/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 3: Test end-to-end**

```bash
./qt/build/snapforge-qt
```

Test sequence:
1. Cmd+Shift+S → overlay appears
2. Draw a region → enters annotate mode with toolbar
3. Press A → arrow tool selected, draw an arrow
4. Press R → rect tool, draw a rectangle
5. Press T → text tool, click to place text, type, press Enter
6. Press B → blur tool, drag over area
7. Cmd+Z → undo last annotation
8. Enter → save composited image with annotations
9. Verify saved PNG contains the annotations

- [ ] **Step 4: Commit**

```bash
git add qt/src/main.cpp
git commit -m "feat(qt): update main.cpp with composited save/copy via new signals"
```

---

### Task 8: Handle Escape in text input and keyboard event routing

**Files:**
- Modify: `qt/src/AnnotationCanvas.cpp`
- Modify: `qt/src/AnnotationCanvas.h`

- [ ] **Step 1: Add Escape handling for text input**

In `qt/src/AnnotationCanvas.h`, add to the public interface:

```cpp
    // Check if text input is active (to suppress overlay key events)
    bool isTextInputActive() const { return m_waitingForText; }
```

- [ ] **Step 2: Add key event filter for text input Escape**

In `qt/src/AnnotationCanvas.cpp`, add an `eventFilter` to the QLineEdit in `showTextInput()`. After the `connect(m_textInput, &QLineEdit::returnPressed, ...)` line, add:

```cpp
        m_textInput->installEventFilter(this);
```

And add this method to the class. In `qt/src/AnnotationCanvas.h`, add to protected:

```cpp
    bool eventFilter(QObject *obj, QEvent *event) override;
```

In `qt/src/AnnotationCanvas.cpp`, add:

```cpp
bool AnnotationCanvas::eventFilter(QObject *obj, QEvent *event) {
    if (obj == m_textInput && event->type() == QEvent::KeyPress) {
        auto *keyEvent = static_cast<QKeyEvent *>(event);
        if (keyEvent->key() == Qt::Key_Escape) {
            cancelTextInput();
            return true;
        }
    }
    return QWidget::eventFilter(obj, event);
}
```

- [ ] **Step 3: Update OverlayWindow to check text input before handling keys**

In `qt/src/OverlayWindow.cpp`, at the top of `keyPressEvent`, after the Escape check, add:

```cpp
    // Don't handle shortcuts while text input is active
    if (m_canvas && m_canvas->isTextInputActive()) return;
```

Add this right after the Escape block (before the annotate mode block).

- [ ] **Step 4: Build and verify**

```bash
cd /Users/biplav00/Documents/personal/screen/qt && cmake --build build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add qt/src/AnnotationCanvas.h qt/src/AnnotationCanvas.cpp qt/src/OverlayWindow.cpp
git commit -m "fix(qt): handle Escape in text input and suppress shortcuts during text entry"
```
