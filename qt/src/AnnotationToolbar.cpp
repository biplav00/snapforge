#include "AnnotationToolbar.h"
#include "AnnotationState.h"

#include <QHBoxLayout>
#include <QPushButton>
#include <QFrame>
#include <QColorDialog>
#include <QPainter>
#include <QPainterPath>
#include <QMouseEvent>
#include <QScreen>
#include <QApplication>

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

// Map between ToolType enum values (in declaration order) and display symbols.
struct ToolEntry {
    ToolType tool;
    QString  symbol;
};

static const QList<ToolEntry> k_tools = {
    { ToolType::Arrow,      QStringLiteral("\u2197") },  // ↗
    { ToolType::Rect,       QStringLiteral("\u25A1") },  // □
    { ToolType::Circle,     QStringLiteral("\u25CB") },  // ○
    { ToolType::Line,       QStringLiteral("\u2500") },  // ─
    { ToolType::DottedLine, QStringLiteral("\u2504") },  // ┄
    { ToolType::Freehand,   QStringLiteral("\u270E") },  // ✎
    { ToolType::Text,       QStringLiteral("T")      },
    { ToolType::Highlight,  QStringLiteral("\u25AC") },  // ▬
    { ToolType::Blur,       QStringLiteral("\u25A6") },  // ▦
    { ToolType::Steps,      QStringLiteral("\u2460") },  // ①
    { ToolType::ColorPicker,QStringLiteral("\u25C9") },  // ◉
    { ToolType::Measure,    QStringLiteral("\u2194") },  // ↔
};

QFrame *makeSeparator(QWidget *parent = nullptr) {
    auto *sep = new QFrame(parent);
    sep->setFrameShape(QFrame::VLine);
    sep->setFrameShadow(QFrame::Sunken);
    sep->setFixedWidth(1);
    sep->setStyleSheet(QStringLiteral("background: #262d3d;"));
    return sep;
}

// Common style shared by tool / action buttons
static const QString k_baseButtonStyle = QStringLiteral(
    "QPushButton {"
    "  background: transparent;"
    "  color: #8b95ab;"
    "  border: none;"
    "  border-radius: 4px;"
    "  font-size: 14px;"
    "  padding: 0px;"
    "}"
    "QPushButton:hover {"
    "  background: #1c2130;"
    "  color: #e8ecf4;"
    "}"
    "QPushButton:pressed {"
    "  background: #1a2540;"
    "}"
    "QPushButton:checked {"
    "  background: rgba(59, 130, 246, 0.15);"
    "  color: #60a5fa;"
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
    layout->setContentsMargins(10, 6, 10, 6);
    layout->setSpacing(4);

    // ---- Tool buttons ----
    for (const auto &entry : k_tools) {
        auto *btn = new QPushButton(entry.symbol, this);
        btn->setCheckable(true);
        btn->setFixedSize(28, 28);
        btn->setStyleSheet(k_baseButtonStyle);
        btn->setToolTip(entry.symbol);

        const ToolType tool = entry.tool;
        connect(btn, &QPushButton::clicked, this, [this, tool]() {
            m_state->setActiveTool(tool);
        });

        m_toolButtons.append(btn);
        layout->addWidget(btn);
    }

    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Color swatches ----
    for (const QColor &color : k_presetColors) {
        auto *btn = new QPushButton(this);
        btn->setCheckable(true);
        btn->setFixedSize(18, 18);
        btn->setToolTip(color.name());

        const QString css = QStringLiteral(
            "QPushButton {"
            "  background: %1;"
            "  border: 2px solid #3a4460;"
            "  border-radius: 3px;"
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
        layout->addWidget(btn);
    }

    // Custom color button
    m_customColorBtn = new QPushButton(QStringLiteral("+"), this);
    m_customColorBtn->setCheckable(true);
    m_customColorBtn->setFixedSize(18, 18);
    m_customColorBtn->setToolTip(QStringLiteral("Custom color"));
    m_customColorBtn->setStyleSheet(
        QStringLiteral(
            "QPushButton {"
            "  background: qlineargradient(x1:0,y1:0,x2:1,y2:1,"
            "    stop:0 #ff0000, stop:0.17 #ff8800, stop:0.33 #ffff00,"
            "    stop:0.5 #00ff00, stop:0.67 #0088ff, stop:0.83 #8800ff, stop:1 #ff0088);"
            "  border: 2px solid #3a4460;"
            "  border-radius: 3px;"
            "  font-size: 11px;"
            "  color: white;"
            "}"
            "QPushButton:hover { border: 2px solid rgba(255,255,255,0.5); }"
            "QPushButton:checked { border: 2px solid white; }"
        )
    );
    connect(m_customColorBtn, &QPushButton::clicked, this, &AnnotationToolbar::onCustomColorClicked);
    layout->addWidget(m_customColorBtn);

    // Keep swatch selection in sync when color changes externally (e.g. color picker tool)
    connect(m_state, &AnnotationState::colorChanged, this, [this](QColor) {
        updateColorSwatches();
    });

    layout->addSpacing(2);
    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Stroke width buttons ----
    // Each button draws a filled circle scaled to the stroke width to give a
    // visual preview.  We use Unicode bullet chars of increasing visual weight
    // as labels alongside a stylesheet font-size trick.
    static const QList<QString> k_strokeSymbols = {
        QStringLiteral("\u2022"),  // • small dot  (width 1)
        QStringLiteral("\u25CF"),  // ● medium dot (width 2)
        QStringLiteral("\u2B24"),  // ⬤ large dot  (width 4)
    };
    static const QList<int> k_strokeFontSizes = { 8, 13, 18 };

    for (int i = 0; i < k_strokeWidths.size(); ++i) {
        const int w = k_strokeWidths[i];
        auto *btn = new QPushButton(k_strokeSymbols[i], this);
        btn->setCheckable(true);
        btn->setFixedSize(28, 28);
        btn->setToolTip(QStringLiteral("Stroke width %1").arg(w));

        const QString css = QStringLiteral(
            "QPushButton {"
            "  background: transparent;"
            "  color: #8b95ab;"
            "  border: none;"
            "  border-radius: 4px;"
            "  font-size: %1px;"
            "  padding: 0px;"
            "}"
            "QPushButton:hover { background: #1c2130; color: #e8ecf4; }"
            "QPushButton:pressed { background: #1a2540; }"
            "QPushButton:checked { background: rgba(59, 130, 246, 0.15); color: #60a5fa; }"
        ).arg(k_strokeFontSizes[i]);
        btn->setStyleSheet(css);

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
    m_undoBtn = new QPushButton(QStringLiteral("\u21B6"), this);  // ↶
    m_undoBtn->setFixedSize(28, 28);
    m_undoBtn->setToolTip(QStringLiteral("Undo"));
    m_undoBtn->setStyleSheet(k_baseButtonStyle);
    connect(m_undoBtn, &QPushButton::clicked, m_state, &AnnotationState::undo);
    layout->addWidget(m_undoBtn);

    m_redoBtn = new QPushButton(QStringLiteral("\u21B7"), this);  // ↷
    m_redoBtn->setFixedSize(28, 28);
    m_redoBtn->setToolTip(QStringLiteral("Redo"));
    m_redoBtn->setStyleSheet(k_baseButtonStyle);
    connect(m_redoBtn, &QPushButton::clicked, m_state, &AnnotationState::redo);
    layout->addWidget(m_redoBtn);

    layout->addSpacing(2);
    layout->addWidget(makeSeparator(this));
    layout->addSpacing(2);

    // ---- Save / Copy / Cancel ----
    auto *saveBtn = new QPushButton(QStringLiteral("Save"), this);
    saveBtn->setFixedHeight(28);
    saveBtn->setStyleSheet(
        QStringLiteral(
            "QPushButton { background: #3b82f6; color: white; border: none;"
            " border-radius: 4px; font-size: 11px; font-weight: 600; padding: 0 10px;"
            " font-family: 'JetBrains Mono', monospace; }"
            "QPushButton:hover { background: #60a5fa; }"
            "QPushButton:pressed { background: #2563eb; }"
        )
    );
    connect(saveBtn, &QPushButton::clicked, this, &AnnotationToolbar::saveRequested);
    layout->addWidget(saveBtn);

    auto *copyBtn = new QPushButton(QStringLiteral("Copy"), this);
    copyBtn->setFixedHeight(28);
    copyBtn->setStyleSheet(
        QStringLiteral(
            "QPushButton { background: #1e2330; color: #8b95ab; border: 1px solid #262d3d;"
            " border-radius: 4px; font-size: 11px; font-weight: 600; padding: 0 10px;"
            " font-family: 'JetBrains Mono', monospace; }"
            "QPushButton:hover { background: #1c2130; color: #e8ecf4; border-color: #3a4460; }"
            "QPushButton:pressed { background: #1a2540; }"
        )
    );
    connect(copyBtn, &QPushButton::clicked, this, &AnnotationToolbar::copyRequested);
    layout->addWidget(copyBtn);

    auto *cancelBtn = new QPushButton(QStringLiteral("Cancel"), this);
    cancelBtn->setFixedHeight(28);
    cancelBtn->setStyleSheet(
        QStringLiteral(
            "QPushButton { background: transparent; color: #3d4660; border: none;"
            " border-radius: 4px; font-size: 13px; padding: 0 6px; }"
            "QPushButton:hover { color: #8b95ab; }"
            "QPushButton:pressed { color: #e8ecf4; }"
        )
    );
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
    if (m_customColorBtn) {
        m_customColorBtn->setChecked(!matchesPreset);
    }
}

// ---------------------------------------------------------------------------
// Slot handlers
// ---------------------------------------------------------------------------

void AnnotationToolbar::onColorSwatchClicked(const QColor &color)
{
    m_state->setStrokeColor(color);
    updateColorSwatches();
}

void AnnotationToolbar::onCustomColorClicked()
{
    const QColor chosen = QColorDialog::getColor(
        m_state->strokeColor(), this, QStringLiteral("Choose color"),
        QColorDialog::ShowAlphaChannel
    );
    if (chosen.isValid()) {
        m_state->setStrokeColor(chosen);
        updateColorSwatches();
    }
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

    // Determine available screen height at that position
    QScreen *screen = QApplication::primaryScreen();
    if (screen) {
        const QRect avail = screen->availableGeometry();

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
    QWidget::mousePressEvent(event);
}

void AnnotationToolbar::mouseMoveEvent(QMouseEvent *event)
{
    if (m_dragging) {
        const QPoint delta = event->globalPosition().toPoint() - m_dragStartGlobal;
        move(m_dragStartPos + delta);
    }
    QWidget::mouseMoveEvent(event);
}

void AnnotationToolbar::mouseReleaseEvent(QMouseEvent *event)
{
    if (event->button() == Qt::LeftButton) {
        m_dragging = false;
    }
    QWidget::mouseReleaseEvent(event);
}

// ---------------------------------------------------------------------------
// Painting — dark semi-transparent rounded-rect background
// ---------------------------------------------------------------------------

void AnnotationToolbar::paintEvent(QPaintEvent * /*event*/)
{
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    QPainterPath path;
    path.addRoundedRect(rect(), 10, 10);

    p.fillPath(path, QColor(12, 14, 18, 242));

    // Subtle border
    QPen borderPen(QColor(38, 45, 61, 180));
    borderPen.setWidth(1);
    p.setPen(borderPen);
    p.drawPath(path);
}
