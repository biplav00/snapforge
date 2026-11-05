#include "AnnotationToolbar.h"
#include "AnnotationState.h"

#include <QHBoxLayout>
#include <QPushButton>
#include <QFrame>
#include <QColorDialog>
#include <QIcon>
#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <QMouseEvent>
#include <QScreen>
#include <QApplication>
#include <QPointer>

#ifdef Q_OS_MACOS
#include <objc/runtime.h>
#include <objc/message.h>
#endif

// NSWindow level of the capture overlay (OverlayWindow::kOverlayWindowLevel).
// Popups the toolbar opens must sit above it or they appear behind the
// full-screen overlay.
static constexpr long kAboveOverlayLevel = 1001;

// ---------------------------------------------------------------------------
// Static constant definitions
// ---------------------------------------------------------------------------

const QList<QColor> AnnotationToolbar::k_presetColors = {
    QColor("#FF0000"),
    QColor("#4A9EFF"),
    QColor("#00CC00"),
    QColor("#FFCC00"),
    QColor("#FF6600"),
    QColor("#9933FF"),
    QColor("#FFFFFF"),
    QColor("#000000"),
};

const QList<int> AnnotationToolbar::k_strokeWidths = { 1, 2, 4 };

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

namespace {

// Sizing scale for the compact (28 px) toolbar.
constexpr int kButtonSize = 28;
constexpr int kIconSize = 16;
constexpr int kSwatchSize = 18;
constexpr int kSeparatorHeight = 18;

// Icon palette, matching the toolbar's button states.
const QColor kIconNormal("#8b95ab");
const QColor kIconHover("#e8ecf4");
const QColor kIconChecked("#60a5fa");
const QColor kIconDisabled("#3d4660");
const QColor kIconAccent("#8bbcff");

enum class IconKind {
    Arrow, Rect, Circle, Line, DottedLine, Freehand, Text,
    Highlight, Blur, Steps, ColorPicker, Measure,
    StrokeThin, StrokeMedium, StrokeThick,
    Undo, Redo, Check, Close
};

// Map between ToolType enum values (in declaration order) and display icons.
struct ToolEntry {
    ToolType tool;
    IconKind icon;
    const char *name;
};

static const QList<ToolEntry> k_tools = {
    { ToolType::Arrow,       IconKind::Arrow,      "Arrow" },
    { ToolType::Rect,        IconKind::Rect,       "Rectangle" },
    { ToolType::Circle,      IconKind::Circle,     "Circle" },
    { ToolType::Line,        IconKind::Line,       "Line" },
    { ToolType::DottedLine,  IconKind::DottedLine, "Dotted line" },
    { ToolType::Freehand,    IconKind::Freehand,   "Freehand" },
    { ToolType::Text,        IconKind::Text,       "Text" },
    { ToolType::Highlight,   IconKind::Highlight,  "Highlight" },
    { ToolType::Blur,        IconKind::Blur,       "Blur" },
    { ToolType::Steps,       IconKind::Steps,      "Step numbers" },
    { ToolType::ColorPicker, IconKind::ColorPicker, "Color picker" },
    { ToolType::Measure,     IconKind::Measure,    "Measure" },
};

// ---------------------------------------------------------------------------
// Vector icons
//
// Drawn in a 24x24 coordinate space and rendered into QPixmaps at 1x/2x/3x,
// so they stay sharp on Retina displays without needing an SVG module.
// ---------------------------------------------------------------------------

void drawIcon(QPainter &p, IconKind kind, const QColor &color)
{
    auto pen = [&color](qreal width) {
        QPen p(color);
        p.setWidthF(width);
        p.setCapStyle(Qt::RoundCap);
        p.setJoinStyle(Qt::RoundJoin);
        return p;
    };

    p.setBrush(Qt::NoBrush);

    switch (kind) {
    case IconKind::Arrow: {
        p.setPen(pen(1.8));
        p.drawLine(QPointF(5.5, 18.5), QPointF(18.5, 5.5));
        QPainterPath head;
        head.moveTo(10.5, 5.5);
        head.lineTo(18.5, 5.5);
        head.lineTo(18.5, 13.5);
        p.drawPath(head);
        break;
    }
    case IconKind::Rect: {
        p.setPen(pen(1.8));
        p.drawRoundedRect(QRectF(4.5, 6.5, 15, 11), 1.6, 1.6);
        break;
    }
    case IconKind::Circle: {
        p.setPen(pen(1.8));
        p.drawEllipse(QPointF(12, 12), 7.5, 6.5);
        break;
    }
    case IconKind::Line: {
        p.setPen(pen(1.8));
        p.drawLine(QPointF(5, 19), QPointF(19, 5));
        break;
    }
    case IconKind::DottedLine: {
        QPen dotted = pen(1.8);
        // Qt dash lengths are multiples of the pen width; convert from the
        // 24x24 design's 1.5 / 4 user-unit dash pattern.
        dotted.setDashPattern({0.83, 2.22});
        p.setPen(dotted);
        p.drawLine(QPointF(5, 19), QPointF(19, 5));
        break;
    }
    case IconKind::Freehand: {
        p.setPen(pen(1.6));
        QPainterPath path;
        path.moveTo(4.5, 19.5);
        path.lineTo(8.1, 18.7);
        path.lineTo(18.6, 8.2);
        path.cubicTo(19.1, 7.7, 19.1, 6.9, 18.6, 6.4);
        path.lineTo(18.5, 6.3);
        path.cubicTo(18.0, 5.8, 17.2, 5.8, 16.7, 6.3);
        path.lineTo(5.3, 15.9);
        path.closeSubpath();
        p.drawPath(path);
        p.drawLine(QPointF(14.3, 7.0), QPointF(17.0, 9.7));
        break;
    }
    case IconKind::Text: {
        p.setPen(pen(1.8));
        p.drawLine(QPointF(5, 6.5), QPointF(19, 6.5));
        p.drawLine(QPointF(12, 6.5), QPointF(12, 18));
        p.drawLine(QPointF(9, 18), QPointF(15, 18));
        break;
    }
    case IconKind::Highlight: {
        p.setPen(pen(1.6));
        QPainterPath path;
        path.moveTo(6.0, 15.8);
        path.lineTo(14.6, 7.0);
        path.lineTo(17.3, 9.7);
        path.lineTo(8.6, 18.4);
        path.lineTo(6.0, 19.0);
        path.closeSubpath();
        p.drawPath(path);
        p.drawLine(QPointF(15.1, 6.5), QPointF(17.5, 8.9));
        p.drawLine(QPointF(5, 21), QPointF(19, 21));
        break;
    }
    case IconKind::Blur: {
        static const qreal kOpacity[3][3] = {
            { .34, .62, .30 },
            { .62, .92, .58 },
            { .30, .58, .30 },
        };
        static const qreal kPos[3] = { 4.0, 9.8, 15.6 };

        p.setPen(Qt::NoPen);
        for (int row = 0; row < 3; ++row) {
            for (int col = 0; col < 3; ++col) {
                QColor cell = color;
                cell.setAlphaF(cell.alphaF() * kOpacity[row][col]);
                p.setBrush(cell);
                p.drawRoundedRect(QRectF(kPos[col], kPos[row], 4.4, 4.4), 1, 1);
            }
        }
        break;
    }
    case IconKind::Steps: {
        p.setPen(pen(1.6));
        p.drawEllipse(QPointF(12, 12), 8, 8);
        QPainterPath one;
        one.moveTo(10.7, 9.4);
        one.lineTo(12.4, 8.0);
        one.lineTo(12.4, 16.0);
        p.drawPath(one);
        break;
    }
    case IconKind::ColorPicker: {
        p.setPen(pen(1.55));
        QPainterPath path;
        path.moveTo(4.5, 19.5);
        path.lineTo(5.3, 16.1);
        path.lineTo(14.9, 6.4);
        path.lineTo(17.4, 3.9);
        path.cubicTo(17.8, 3.5, 18.4, 3.5, 18.8, 3.9);
        path.lineTo(20.1, 5.2);
        path.cubicTo(20.5, 5.6, 20.5, 6.2, 20.1, 6.6);
        path.lineTo(17.6, 9.1);
        path.lineTo(8.0, 18.8);
        path.closeSubpath();
        p.drawPath(path);
        p.drawLine(QPointF(14.7, 5.6), QPointF(18.4, 9.3));
        p.drawLine(QPointF(5.3, 16.1), QPointF(8.0, 18.8));
        break;
    }
    case IconKind::Measure: {
        p.setPen(pen(1.55));
        QPainterPath path;
        path.moveTo(4.0, 15.2);
        path.lineTo(15.2, 4.0);
        path.lineTo(20.0, 8.8);
        path.lineTo(8.8, 20.0);
        path.closeSubpath();
        p.drawPath(path);
        p.drawLine(QPointF(7.3, 15.1), QPointF(9.0, 13.4));
        p.drawLine(QPointF(10.2, 12.2), QPointF(11.9, 10.5));
        p.drawLine(QPointF(13.1, 9.3), QPointF(14.8, 7.6));
        p.drawLine(QPointF(16.0, 6.4), QPointF(17.7, 4.7));
        break;
    }
    case IconKind::StrokeThin:
    case IconKind::StrokeMedium:
    case IconKind::StrokeThick: {
        const qreal width = kind == IconKind::StrokeThin    ? 2.25
                          : kind == IconKind::StrokeMedium  ? 3.75
                                                            : 6.0;
        p.setPen(pen(width));
        p.drawLine(QPointF(1.5, 12), QPointF(22.5, 12));
        break;
    }
    case IconKind::Undo: {
        p.setPen(pen(1.8));
        QPainterPath arrow;
        arrow.moveTo(9.0, 7.0);
        arrow.lineTo(4.5, 11.5);
        arrow.lineTo(9.0, 16.0);
        p.drawPath(arrow);

        QPainterPath curve;
        curve.moveTo(4.5, 11.5);
        curve.lineTo(13.2, 11.5);
        curve.arcTo(QRectF(7.9, 11.5, 10.6, 10.6), 90, -90);
        curve.lineTo(18.5, 17.5);
        p.drawPath(curve);
        break;
    }
    case IconKind::Redo: {
        p.setPen(pen(1.8));
        QPainterPath arrow;
        arrow.moveTo(15.0, 7.0);
        arrow.lineTo(19.5, 11.5);
        arrow.lineTo(15.0, 16.0);
        p.drawPath(arrow);

        QPainterPath curve;
        curve.moveTo(19.5, 11.5);
        curve.lineTo(10.8, 11.5);
        curve.arcTo(QRectF(5.5, 11.5, 10.6, 10.6), 90, 90);
        curve.lineTo(5.5, 17.5);
        p.drawPath(curve);
        break;
    }
    case IconKind::Check: {
        p.setPen(pen(2.0));
        QPainterPath check;
        check.moveTo(5.0, 12.5);
        check.lineTo(9.5, 17.0);
        check.lineTo(19.0, 7.5);
        p.drawPath(check);
        break;
    }
    case IconKind::Close: {
        p.setPen(pen(1.8));
        p.drawLine(QPointF(6, 6), QPointF(18, 18));
        p.drawLine(QPointF(18, 6), QPointF(6, 18));
        break;
    }
    }
}

QPixmap renderIcon(IconKind kind, const QColor &color, int logicalSize, int dpr)
{
    QPixmap pm(logicalSize * dpr, logicalSize * dpr);
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);

    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.scale(logicalSize / 24.0, logicalSize / 24.0);
    drawIcon(p, kind, color);
    return pm;
}

QIcon makeIcon(IconKind kind, int logicalSize = kIconSize,
               const QColor &normal = kIconNormal,
               const QColor &hover = kIconHover,
               const QColor &checked = kIconChecked,
               const QColor &disabled = kIconDisabled)
{
    QIcon icon;

    struct Variant {
        QIcon::Mode mode;
        QIcon::State state;
        QColor color;
    };
    const Variant variants[] = {
        { QIcon::Normal,   QIcon::Off, normal },
        { QIcon::Active,   QIcon::Off, hover },
        { QIcon::Normal,   QIcon::On,  checked },
        { QIcon::Active,   QIcon::On,  kIconAccent },
        { QIcon::Disabled, QIcon::Off, disabled },
        { QIcon::Disabled, QIcon::On,  disabled },
    };

    for (const Variant &v : variants) {
        for (int dpr = 1; dpr <= 3; ++dpr) {
            icon.addPixmap(renderIcon(kind, v.color, logicalSize, dpr),
                           v.mode, v.state);
        }
    }
    return icon;
}

QFrame *makeSeparator(QWidget *parent = nullptr) {
    auto *sep = new QFrame(parent);
    sep->setFixedSize(1, kSeparatorHeight);
    sep->setStyleSheet(QStringLiteral("background: #283041; border: none;"));
    return sep;
}

// Common style shared by tool / action buttons
static const QString k_baseButtonStyle = QStringLiteral(
    "QPushButton {"
    "  background: transparent;"
    "  color: #8b95ab;"
    "  border: 1px solid transparent;"
    "  border-radius: 7px;"
    "  font-size: 14px;"
    "  padding: 0px;"
    "}"
    "QPushButton:hover {"
    "  background: #1b2230;"
    "  color: #e8ecf4;"
    "}"
    "QPushButton:pressed {"
    "  background: #1a2540;"
    "}"
    "QPushButton:checked {"
    "  background: rgba(59, 130, 246, 0.16);"
    "  color: #8bbcff;"
    "  border: 1px solid rgba(96, 165, 250, 0.25);"
    "}"
    "QPushButton:disabled {"
    "  color: #3d4660;"
    "}"
);

} // namespace


// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

AnnotationToolbar::AnnotationToolbar(AnnotationState *state, QWidget *parent)
    : QWidget(parent)
    , m_state(state)
{
    setAttribute(Qt::WA_TranslucentBackground);
    // Presses on the toolbar background, on its margins, or on a DISABLED
    // button (undo/redo with nothing to undo) would otherwise bubble up to
    // the OverlayWindow, which treats them as a click outside the region.
    setAttribute(Qt::WA_NoMousePropagation);

    buildUi();

    // Keep undo/redo enabled state in sync with history changes
    connect(m_state, &AnnotationState::changed, this, &AnnotationToolbar::updateUndoRedo);

    // Keep tool button checked state in sync when tool changes externally
    connect(m_state, &AnnotationState::toolChanged, this, &AnnotationToolbar::updateToolButtons);

    // Initial state
    updateToolButtons();
    updateUndoRedo();
}

// ---------------------------------------------------------------------------
// UI construction
// ---------------------------------------------------------------------------

void AnnotationToolbar::buildUi()
{
    auto *layout = new QHBoxLayout(this);
    layout->setContentsMargins(8, 6, 8, 6);
    layout->setSpacing(3);

    // ---- Tool buttons ----
    for (const auto &entry : k_tools) {
        auto *btn = new QPushButton(this);
        btn->setCheckable(true);
        btn->setFixedSize(kButtonSize, kButtonSize);
        btn->setIcon(makeIcon(entry.icon));
        btn->setIconSize(QSize(kIconSize, kIconSize));
        btn->setStyleSheet(k_baseButtonStyle);
        btn->setAccessibleName(QString::fromLatin1(entry.name));
        btn->setToolTip(QString::fromLatin1(entry.name));

        const ToolType tool = entry.tool;
        connect(btn, &QPushButton::clicked, this, [this, tool]() {
            // Toggle: clicking the active tool deselects it.
            if (m_state->activeTool() == tool) {
                m_state->setActiveTool(ToolType::None);
            } else {
                m_state->setActiveTool(tool);
            }
        });

        m_toolButtons.append(btn);
        layout->addWidget(btn);
    }

    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Color swatches ----
    // A tighter sub-row keeps the swatch gaps at 5 px while tool buttons use
    // the 3 px compact rhythm.
    auto *swatchRow = new QWidget(this);
    auto *swatchLayout = new QHBoxLayout(swatchRow);
    swatchLayout->setContentsMargins(0, 0, 0, 0);
    swatchLayout->setSpacing(5);

    for (const QColor &color : k_presetColors) {
        auto *btn = new QPushButton(swatchRow);
        btn->setCheckable(true);
        btn->setFixedSize(kSwatchSize, kSwatchSize);
        btn->setToolTip(color.name());

        const QString css = QStringLiteral(
            "QPushButton {"
            "  background: %1;"
            "  border: 2px solid #3a4460;"
            "  border-radius: 4px;"
            "}"
            "QPushButton:hover {"
            "  border: 2px solid rgba(255,255,255,0.5);"
            "}"
            "QPushButton:checked {"
            "  border: 2px solid white;"
            "}"
        ).arg(color.name());
        btn->setStyleSheet(css);

        connect(btn, &QPushButton::clicked, this, [this, color]() {
            onColorSwatchClicked(color);
        });

        m_colorSwatches.append(btn);
        swatchLayout->addWidget(btn);
    }
    layout->addWidget(swatchRow);

    // Keep swatch selection in sync when color changes externally (e.g. color picker tool)
    connect(m_state, &AnnotationState::colorChanged, this, [this](QColor) {
        updateColorSwatches();
    });

    layout->addSpacing(2);
    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Stroke width buttons ----
    // Each button draws a horizontal bar whose thickness previews the stroke
    // width, so the three choices read at a glance without font-size hacks.
    static const QList<IconKind> k_strokeIcons = {
        IconKind::StrokeThin,
        IconKind::StrokeMedium,
        IconKind::StrokeThick,
    };

    for (int i = 0; i < k_strokeWidths.size(); ++i) {
        const int w = k_strokeWidths[i];
        auto *btn = new QPushButton(this);
        btn->setCheckable(true);
        btn->setFixedSize(kButtonSize, kButtonSize);
        btn->setIcon(makeIcon(k_strokeIcons[i]));
        btn->setIconSize(QSize(kIconSize, kIconSize));
        btn->setToolTip(QStringLiteral("Stroke width %1").arg(w));
        btn->setStyleSheet(k_baseButtonStyle);

        connect(btn, &QPushButton::clicked, this, [this, w]() {
            onStrokeWidthClicked(w);
        });

        m_strokeButtons.append(btn);
        layout->addWidget(btn);
    }

    // Update stroke button checked state when width changes externally
    connect(m_state, &AnnotationState::strokeWidthChanged, this, [this](int width) {
        for (int i = 0; i < m_strokeButtons.size(); ++i) {
            m_strokeButtons[i]->setChecked(k_strokeWidths[i] == width);
        }
    });
    // Initial checked state
    for (int i = 0; i < m_strokeButtons.size(); ++i) {
        m_strokeButtons[i]->setChecked(k_strokeWidths[i] == m_state->strokeWidth());
    }

    layout->addSpacing(2);
    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Undo / Redo ----
    m_undoBtn = new QPushButton(this);
    m_undoBtn->setFixedSize(kButtonSize, kButtonSize);
    m_undoBtn->setIcon(makeIcon(IconKind::Undo));
    m_undoBtn->setIconSize(QSize(kIconSize, kIconSize));
    m_undoBtn->setToolTip(QStringLiteral("Undo"));
    m_undoBtn->setStyleSheet(k_baseButtonStyle);
    connect(m_undoBtn, &QPushButton::clicked, m_state, &AnnotationState::undo);
    layout->addWidget(m_undoBtn);

    m_redoBtn = new QPushButton(this);
    m_redoBtn->setFixedSize(kButtonSize, kButtonSize);
    m_redoBtn->setIcon(makeIcon(IconKind::Redo));
    m_redoBtn->setIconSize(QSize(kIconSize, kIconSize));
    m_redoBtn->setToolTip(QStringLiteral("Redo"));
    m_redoBtn->setStyleSheet(k_baseButtonStyle);
    connect(m_redoBtn, &QPushButton::clicked, m_state, &AnnotationState::redo);
    layout->addWidget(m_redoBtn);

    layout->addSpacing(2);
    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Save / Cancel ----
    auto *saveBtn = new QPushButton(QStringLiteral("Save"), this);
    saveBtn->setFixedHeight(kButtonSize);
    saveBtn->setMinimumWidth(62);
    saveBtn->setIcon(makeIcon(IconKind::Check, 14,
                              QColor("#ffffff"), QColor("#ffffff"),
                              QColor("#ffffff"), QColor("#c7d2fe")));
    saveBtn->setIconSize(QSize(14, 14));
    saveBtn->setToolTip(QStringLiteral("Save and copy"));
    saveBtn->setStyleSheet(
        QStringLiteral(
            "QPushButton { background: #3b82f6; color: white; border: none;"
            " border-radius: 7px; font-size: 12px; font-weight: 600; padding: 0 10px; }"
            "QPushButton:hover { background: #4b8ef8; }"
            "QPushButton:pressed { background: #2563eb; }"
        )
    );
    connect(saveBtn, &QPushButton::clicked, this, &AnnotationToolbar::saveRequested);
    layout->addWidget(saveBtn);

    auto *cancelBtn = new QPushButton(this);
    cancelBtn->setFixedSize(kButtonSize, kButtonSize);
    cancelBtn->setIcon(makeIcon(IconKind::Close, kIconSize,
                                QColor("#778196"), QColor("#ff8b8e"),
                                QColor("#ff8b8e"), QColor("#3d4660")));
    cancelBtn->setIconSize(QSize(kIconSize, kIconSize));
    cancelBtn->setToolTip(QStringLiteral("Cancel (Esc)"));
    cancelBtn->setStyleSheet(k_baseButtonStyle);
    connect(cancelBtn, &QPushButton::clicked, this, &AnnotationToolbar::cancelRequested);
    layout->addWidget(cancelBtn);

    setLayout(layout);
    adjustSize();
}

// ---------------------------------------------------------------------------
// State sync helpers
// ---------------------------------------------------------------------------

void AnnotationToolbar::updateToolButtons()
{
    const ToolType active = m_state->activeTool();
    for (int i = 0; i < m_toolButtons.size(); ++i) {
        m_toolButtons[i]->setChecked(k_tools[i].tool == active);
    }
}

void AnnotationToolbar::updateUndoRedo()
{
    if (m_undoBtn) m_undoBtn->setEnabled(m_state->canUndo());
    if (m_redoBtn) m_redoBtn->setEnabled(m_state->canRedo());
}

void AnnotationToolbar::updateColorSwatches()
{
    const QColor current = m_state->strokeColor();
    bool matchesPreset = false;
    for (int i = 0; i < m_colorSwatches.size(); ++i) {
        bool match = (k_presetColors[i] == current);
        m_colorSwatches[i]->setChecked(match);
        if (match) matchesPreset = true;
    }
    // Highlight custom color button when using a non-preset color
    // (no custom color button anymore - always show as unchecked)
}

// ---------------------------------------------------------------------------
// Slot handlers
// ---------------------------------------------------------------------------

void AnnotationToolbar::onColorSwatchClicked(const QColor &color)
{
    m_state->setStrokeColor(color);
    updateColorSwatches();
}

void AnnotationToolbar::onStrokeWidthClicked(int width)
{
    m_state->setStrokeWidth(width);
    for (int i = 0; i < m_strokeButtons.size(); ++i) {
        m_strokeButtons[i]->setChecked(k_strokeWidths[i] == width);
    }
}

// ---------------------------------------------------------------------------
// Positioning
// ---------------------------------------------------------------------------

void AnnotationToolbar::positionRelativeTo(int regionX, int regionY, int regionW, int regionH)
{
    adjustSize();
    const QSize sz = sizeHint();

    // Center horizontally below the region, with a small gap
    static const int kGap = 8;
    int x = regionX + (regionW - sz.width()) / 2;
    int y = regionY + regionH + kGap;

    // The toolbar is a CHILD of the overlay window, so move() is parent-local —
    // clamp against the parent overlay's rect, not the primary screen's
    // (global) available geometry, which is a different coordinate space and
    // possibly a different display entirely.
    if (const QWidget *overlay = parentWidget()) {
        const QRect avail = overlay->rect();

        // Clamp horizontally
        if (x < avail.left()) x = avail.left();
        if (x + sz.width() > avail.right()) x = avail.right() - sz.width();

        // If it would go off the bottom, place it above the region instead
        if (y + sz.height() > avail.bottom()) {
            y = regionY - sz.height() - kGap;
        }

        // If even above the region is off-screen, clamp to top
        if (y < avail.top()) y = avail.top();
    }

    move(x, y);
}

// ---------------------------------------------------------------------------
// Drag support
// ---------------------------------------------------------------------------

void AnnotationToolbar::mousePressEvent(QMouseEvent *event)
{
    if (event->button() == Qt::LeftButton) {
        // Only start drag if the press is on the widget itself, not a child button
        if (childAt(event->pos()) == nullptr) {
            m_dragging = true;
            m_dragStartPos    = pos();
            m_dragStartGlobal = event->globalPosition().toPoint();
        }
    }
    // Accept (QWidget's default handler ignores): the toolbar owns every
    // press inside it, it's never an "outside the region" click.
    event->accept();
}

void AnnotationToolbar::mouseMoveEvent(QMouseEvent *event)
{
    if (m_dragging) {
        const QPoint delta = event->globalPosition().toPoint() - m_dragStartGlobal;
        move(m_dragStartPos + delta);
    }
    event->accept();
}

void AnnotationToolbar::mouseReleaseEvent(QMouseEvent *event)
{
    if (event->button() == Qt::LeftButton) {
        m_dragging = false;
    }
    event->accept();
}

// ---------------------------------------------------------------------------
// Painting — dark semi-transparent rounded-rect background
// ---------------------------------------------------------------------------

void AnnotationToolbar::paintEvent(QPaintEvent * /*event*/)
{
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    QPainterPath path;
    path.addRoundedRect(rect(), 11, 11);

    p.fillPath(path, QColor(12, 14, 18, 242));

    // Subtle border
    QPen borderPen(QColor(255, 255, 255, 26));
    borderPen.setWidth(1);
    p.setPen(borderPen);
    p.drawPath(path);
}
