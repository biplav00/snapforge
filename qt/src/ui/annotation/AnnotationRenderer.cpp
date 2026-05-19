#include "AnnotationRenderer.h"

#include <QPainterPath>
#include <QFontMetrics>
#include <QFont>
#include <QColor>
#include <cmath>

namespace AnnotationRenderer {

static void renderArrow(QPainter &p, const ArrowData &d, const QColor &color, int strokeWidth) {
    double sw = static_cast<double>(strokeWidth);

    QPen pen(color, sw, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));

    double headLen = std::max(10.0, sw * 4.0);
    double angle = std::atan2(d.endY - d.startY, d.endX - d.startX);

    QPointF tip(d.endX, d.endY);
    QPointF wing1(
        d.endX - headLen * std::cos(angle - M_PI / 6.0),
        d.endY - headLen * std::sin(angle - M_PI / 6.0)
    );
    QPointF wing2(
        d.endX - headLen * std::cos(angle + M_PI / 6.0),
        d.endY - headLen * std::sin(angle + M_PI / 6.0)
    );

    QPainterPath arrowHead;
    arrowHead.moveTo(tip);
    arrowHead.lineTo(wing1);
    arrowHead.lineTo(wing2);
    arrowHead.closeSubpath();

    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawPath(arrowHead);
}

static void renderRect(QPainter &p, const RectData &d, const QColor &color, int strokeWidth) {
    QPen pen(color, strokeWidth, Qt::SolidLine, Qt::SquareCap, Qt::MiterJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawRect(QRectF(d.x, d.y, d.width, d.height));
}

static void renderCircle(QPainter &p, const CircleData &d, const QColor &color, int strokeWidth) {
    QPen pen(color, strokeWidth, Qt::SolidLine);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawEllipse(QPointF(d.cx, d.cy), d.rx, d.ry);
}

static void renderLine(QPainter &p, const LineData &d, const QColor &color, int strokeWidth) {
    QPen pen(color, strokeWidth, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));
}

static void renderDottedLine(QPainter &p, const DottedLineData &d, const QColor &color, int strokeWidth) {
    double sw = static_cast<double>(strokeWidth);
    QPen pen(color, strokeWidth, Qt::CustomDashLine, Qt::RoundCap, Qt::RoundJoin);
    pen.setDashPattern({sw * 3.0, sw * 3.0});
    p.setPen(pen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));
}

static void renderFreehand(QPainter &p, const FreehandData &d, const QColor &color, int strokeWidth) {
    if (d.points.size() < 2)
        return;

    QPainterPath path;
    path.moveTo(d.points[0]);
    for (int i = 1; i < d.points.size(); ++i) {
        path.lineTo(d.points[i]);
    }

    QPen pen(color, strokeWidth, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    p.setBrush(Qt::NoBrush);
    p.drawPath(path);
}

static void renderText(QPainter &p, const TextData &d, const QColor &color) {
    QFont font("system-ui", d.fontSize);
    p.setFont(font);
    p.setPen(color);
    p.drawText(QPointF(d.x, d.y + d.fontSize), d.text);
}

static void renderHighlight(QPainter &p, const HighlightData &d, const QColor &color) {
    p.save();
    p.setOpacity(0.3);
    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.fillRect(QRectF(d.x, d.y, d.width, d.height), color);
    p.restore();
}

static void renderBlur(QPainter &p, const BlurData &d, const QImage &screenshotRegion) {
    if (screenshotRegion.isNull()) {
        // Fallback: dashed rect
        QPen pen(Qt::gray, 1, Qt::DashLine);
        p.setPen(pen);
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(d.x, d.y, d.width, d.height));
        return;
    }

    int blockSize = std::max(2, d.intensity);
    int rx = static_cast<int>(d.x);
    int ry = static_cast<int>(d.y);
    int rw = static_cast<int>(d.width);
    int rh = static_cast<int>(d.height);

    int imgW = screenshotRegion.width();
    int imgH = screenshotRegion.height();

    for (int by = 0; by < rh; by += blockSize) {
        for (int bx = 0; bx < rw; bx += blockSize) {
            // Sample center pixel of block
            int cx = rx + bx + blockSize / 2;
            int cy = ry + by + blockSize / 2;

            // Clamp to image bounds
            cx = std::max(0, std::min(cx, imgW - 1));
            cy = std::max(0, std::min(cy, imgH - 1));

            QColor c = screenshotRegion.pixelColor(cx, cy);
            int blockW = std::min(blockSize, rw - bx);
            int blockH = std::min(blockSize, rh - by);
            p.fillRect(QRectF(d.x + bx, d.y + by, blockW, blockH), c);
        }
    }
}

static void renderSteps(QPainter &p, const StepsData &d, const QColor &color, int strokeWidth) {
    double radius = 8.0 + strokeWidth * 3.0;

    // Filled circle
    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawEllipse(QPointF(d.x, d.y), radius, radius);

    // White bold number centered
    QFont numFont("system-ui", static_cast<int>(radius * 0.9));
    numFont.setBold(true);
    p.setFont(numFont);
    p.setPen(Qt::white);

    QString numStr = QString::number(d.number);
    QFontMetrics fm(numFont);
    QRect textRect = fm.boundingRect(numStr);
    double textX = d.x - textRect.width() / 2.0;
    double textY = d.y + textRect.height() / 2.0 - fm.descent();
    p.drawText(QPointF(textX, textY), numStr);

    // Label rounded rect to the right
    if (!d.label.isEmpty()) {
        QFont labelFont("system-ui", 12);
        p.setFont(labelFont);
        QFontMetrics lfm(labelFont);
        QRect lbr = lfm.boundingRect(d.label);

        double padding = 6.0;
        double lx = d.x + radius + 6.0;
        double ly = d.y - lbr.height() / 2.0 - padding;
        double lw = lbr.width() + padding * 2.0;
        double lh = lbr.height() + padding * 2.0;

        p.setPen(Qt::NoPen);
        p.setBrush(color);
        p.drawRoundedRect(QRectF(lx, ly, lw, lh), 4.0, 4.0);

        p.setPen(Qt::white);
        p.drawText(QPointF(lx + padding, ly + padding + lbr.height() - lfm.descent()), d.label);
    }
}

static void renderMeasure(QPainter &p, const MeasureData &d, const QColor &color, int strokeWidth) {
    // Dashed line
    QPen dashPen(color, strokeWidth, Qt::CustomDashLine, Qt::RoundCap, Qt::RoundJoin);
    dashPen.setDashPattern({6.0, 4.0});
    p.setPen(dashPen);
    p.drawLine(QPointF(d.startX, d.startY), QPointF(d.endX, d.endY));

    // Filled 3px circles at endpoints
    p.setPen(Qt::NoPen);
    p.setBrush(color);
    p.drawEllipse(QPointF(d.startX, d.startY), 3.0, 3.0);
    p.drawEllipse(QPointF(d.endX, d.endY), 3.0, 3.0);

    // Distance label at midpoint
    double dx = d.endX - d.startX;
    double dy = d.endY - d.startY;
    double dist = std::sqrt(dx * dx + dy * dy);
    QString label = QString("%1px").arg(static_cast<int>(std::round(dist)));

    double mx = (d.startX + d.endX) / 2.0;
    double my = (d.startY + d.endY) / 2.0;

    QFont font("Menlo", 12);
    p.setFont(font);
    QFontMetrics fm(font);
    QRect textBounds = fm.boundingRect(label);

    double padding = 5.0;
    double bgX = mx - textBounds.width() / 2.0 - padding;
    double bgY = my - textBounds.height() / 2.0 - padding;
    double bgW = textBounds.width() + padding * 2.0;
    double bgH = textBounds.height() + padding * 2.0;

    // Dark semi-transparent background
    p.setPen(Qt::NoPen);
    p.setBrush(QColor(0, 0, 0, 160));
    p.drawRoundedRect(QRectF(bgX, bgY, bgW, bgH), 3.0, 3.0);

    // White label text
    p.setPen(Qt::white);
    p.drawText(QPointF(bgX + padding, bgY + padding + textBounds.height() - fm.descent()), label);
}

void render(QPainter &p, const Annotation &a, const QImage &screenshotRegion) {
    p.save();
    p.setRenderHint(QPainter::Antialiasing, true);

    std::visit([&](const auto &data) {
        using T = std::decay_t<decltype(data)>;

        if constexpr (std::is_same_v<T, ArrowData>) {
            renderArrow(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, RectData>) {
            renderRect(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, CircleData>) {
            renderCircle(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, LineData>) {
            renderLine(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, DottedLineData>) {
            renderDottedLine(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, FreehandData>) {
            renderFreehand(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, TextData>) {
            renderText(p, data, a.color);
        } else if constexpr (std::is_same_v<T, HighlightData>) {
            renderHighlight(p, data, a.color);
        } else if constexpr (std::is_same_v<T, BlurData>) {
            renderBlur(p, data, screenshotRegion);
        } else if constexpr (std::is_same_v<T, StepsData>) {
            renderSteps(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, MeasureData>) {
            renderMeasure(p, data, a.color, a.strokeWidth);
        } else if constexpr (std::is_same_v<T, ColorPickerData>) {
            // No render
        }
    }, a.data);

    p.restore();
}

} // namespace AnnotationRenderer
