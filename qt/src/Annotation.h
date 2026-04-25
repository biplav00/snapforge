#ifndef ANNOTATION_H
#define ANNOTATION_H

#include <QString>
#include <QColor>
#include <QPointF>
#include <QVector>
#include <variant>

enum class ToolType {
    None,
    Arrow,
    Rect,
    Circle,
    Line,
    DottedLine,
    Freehand,
    Text,
    Highlight,
    Blur,
    Steps,
    ColorPicker,
    Measure
};

struct ArrowData {
    double startX;
    double startY;
    double endX;
    double endY;
};

struct RectData {
    double x;
    double y;
    double width;
    double height;
};

struct CircleData {
    double cx;
    double cy;
    double rx;
    double ry;
};

struct LineData {
    double startX;
    double startY;
    double endX;
    double endY;
};

struct DottedLineData {
    double startX;
    double startY;
    double endX;
    double endY;
};

struct FreehandData {
    QVector<QPointF> points;
};

struct TextData {
    double x;
    double y;
    QString text;
    int fontSize = 16;
};

struct HighlightData {
    double x;
    double y;
    double width;
    double height;
};

struct BlurData {
    double x;
    double y;
    double width;
    double height;
    int intensity = 10;
};

struct StepsData {
    double x;
    double y;
    int number;
    QString label;
};

struct ColorPickerData {
    double x;
    double y;
};

struct MeasureData {
    double startX;
    double startY;
    double endX;
    double endY;
};

using AnnotationData = std::variant<
    ArrowData,
    RectData,
    CircleData,
    LineData,
    DottedLineData,
    FreehandData,
    TextData,
    HighlightData,
    BlurData,
    StepsData,
    ColorPickerData,
    MeasureData
>;

struct Annotation {
    QString id;
    ToolType tool;
    QColor color;
    int strokeWidth;
    AnnotationData data;
};

#endif // ANNOTATION_H
