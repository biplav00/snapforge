#include "PreferencesWindow.h"

#include <QApplication>
#include <QFileDialog>
#include <QFrame>
#include <QHBoxLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QMap>
#include <QScrollArea>
#include <QShowEvent>
#include <QTabWidget>
#include <QTimer>
#include <QVBoxLayout>
#include <QKeyEvent>
#include <QEvent>
#include <QStyle>

#include "snapforge_ffi.h"

// ===========================================================================
// Theme colors
// ===========================================================================

struct Theme {
    QString bg, bgPanel, bgCard, bgInput, bgHover, bgActive;
    QString border, borderStrong, borderSubtle;
    QString text, textSec, textMuted, textFaint;
    QString accent, accentBright, accentGlow;
    QString success;
};

static Theme darkTheme() {
    return {
        "#0c0e12", "#13161c", "#191d26", "#151920", "#1c2130", "#1a2540",
        "#262d3d", "#3a4460", "#1e2330",
        "#e8ecf4", "#8b95ab", "#5a6580", "#3d4660",
        "#3b82f6", "#60a5fa", "rgba(59,130,246,0.15)",
        "#22c55e"
    };
}

static Theme lightTheme() {
    return {
        "#ffffff", "#f8f9fb", "#f0f2f5", "#f5f6f8", "#ebedf2", "#e0e8ff",
        "#d0d5dd", "#b0b8c8", "#e8ebf0",
        "#1a1d24", "#4a5060", "#7a8090", "#a0a8b8",
        "#3b82f6", "#2563eb", "rgba(59,130,246,0.08)",
        "#16a34a"
    };
}

static Theme g_theme = darkTheme();

// Stylesheet generators using current theme
static QString windowStyle() {
    return QStringLiteral(
        "PreferencesWindow { background: %1; color: %2; }"
        "QWidget { background: %1; color: %2; "
        "  font-family: -apple-system, system-ui; font-size: 13px; }"
    ).arg(g_theme.bg, g_theme.text);
}

static QString tabStyle() {
    return QStringLiteral(
        "QTabWidget::pane { border: 1px solid %1; border-top: none; }"
        "QTabBar::tab { background: %2; color: %3; padding: 6px 16px;"
        "  border: 1px solid %1; border-bottom: none; margin-right: 1px;"
        "  font-size: 12px; font-family: 'JetBrains Mono', monospace; }"
        "QTabBar::tab:selected { background: %4; color: %5;"
        "  border-bottom: 2px solid %6; }"
        "QTabBar::tab:hover { color: %7; }"
    ).arg(g_theme.border, g_theme.bgCard, g_theme.textMuted,
          g_theme.bg, g_theme.accentBright, g_theme.accent, g_theme.textSec);
}

static QString lineEditStyle() {
    return QStringLiteral(
        "QLineEdit { background: %1; border: 1px solid %2;"
        "  border-radius: 6px; padding: 8px 12px; color: %3; }"
        "QLineEdit:focus { border: 1px solid %4; }"
    ).arg(g_theme.bgInput, g_theme.border, g_theme.text, g_theme.accent);
}

static QString checkBoxStyle() {
    return QStringLiteral(
        "QCheckBox { spacing: 12px; }"
        "QCheckBox::indicator { width: 40px; height: 22px; border-radius: 11px; }"
        "QCheckBox::indicator:unchecked { background: %1; }"
        "QCheckBox::indicator:checked   { background: %2; }"
    ).arg(g_theme.borderStrong, g_theme.accent);
}

static QString sliderStyle() {
    return QStringLiteral(
        "QSlider::groove:horizontal { background: %1; height: 6px; border-radius: 3px; }"
        "QSlider::handle:horizontal { background: %2; width: 16px; height: 16px;"
        "  border-radius: 8px; margin: -5px 0; }"
        "QSlider::sub-page:horizontal { background: %3; border-radius: 3px; }"
    ).arg(g_theme.border, g_theme.accentBright, g_theme.accent);
}

static QString defaultBtnStyle() {
    return QStringLiteral(
        "QPushButton { background: %1; border: 1px solid %2; border-radius: 6px;"
        "  padding: 8px 16px; color: %3; }"
        "QPushButton:hover { background: %4; }"
        "QPushButton:pressed { background: %5; }"
    ).arg(g_theme.bgCard, g_theme.border, g_theme.text,
          g_theme.bgHover, g_theme.bgInput);
}

static QString saveBtnStyle() {
    return QStringLiteral(
        "QPushButton { background: %1; border: none; border-radius: 6px;"
        "  padding: 6px 20px; margin: 4px 0px; color: white; font-weight: 600; font-size: 12px;"
        "  font-family: 'JetBrains Mono', monospace; }"
        "QPushButton:hover { background: %2; }"
        "QPushButton:pressed { background: %3; }"
    ).arg(g_theme.accent, g_theme.accentBright,
          g_theme.accent == "#3b82f6" ? "#2563eb" : "#1d4ed8");
}

static QString resetBtnStyle() {
    return QStringLiteral(
        "QPushButton { background: %1; border: 1px solid %2; border-radius: 6px;"
        "  padding: 7px 16px; color: %3; }"
        "QPushButton:hover { color: %4; border-color: %5; }"
    ).arg(g_theme.bgInput, g_theme.border, g_theme.textSec,
          g_theme.text, g_theme.borderStrong);
}

// Card button styles (format selection)
static QString cardStyle(bool selected) {
    if (selected)
        return QStringLiteral(
            "QPushButton { background: %1; border: 2px solid %2;"
            "  border-radius: 8px; padding: 10px 20px; color: %3; font-weight: 600; }"
            "QPushButton:hover { background: %1; }"
        ).arg(g_theme.accentGlow, g_theme.accent, g_theme.accentBright);
    return QStringLiteral(
        "QPushButton { background: %1; border: 1px solid %2;"
        "  border-radius: 8px; padding: 11px 20px; color: %3; }"
        "QPushButton:hover { color: %4; border-color: %5; }"
    ).arg(g_theme.bgInput, g_theme.border, g_theme.textSec,
          g_theme.text, g_theme.borderStrong);
}

// Pill button styles (FPS / quality)
static QString pillStyle(bool selected) {
    if (selected)
        return QStringLiteral(
            "QPushButton { background: %1; border: 1px solid %2;"
            "  border-radius: 16px; padding: 6px 16px; color: %3; font-weight: 600; }"
            "QPushButton:hover { background: %1; }"
        ).arg(g_theme.accentGlow, g_theme.accent, g_theme.accentBright);
    return QStringLiteral(
        "QPushButton { background: %1; border: 1px solid %2;"
        "  border-radius: 16px; padding: 6px 16px; color: %3; }"
        "QPushButton:hover { color: %4; border-color: %5; }"
    ).arg(g_theme.bgInput, g_theme.border, g_theme.textSec,
          g_theme.text, g_theme.borderStrong);
}

// Badge widget for displaying a shortcut (e.g. "⌘ ⇧ S")
static QWidget *makeBadges(const QString &shortcut, QWidget *parent) {
    QString badgeStyle = QStringLiteral(
        "QLabel { background: %1; border: 1px solid %2; border-radius: 3px; padding: 0px 4px;"
        "  font-size: 10px; font-family: 'JetBrains Mono', Menlo, monospace; color: %3;"
        "  min-height: 16px; max-height: 16px; }"
    ).arg(g_theme.bgCard, g_theme.border, g_theme.textSec);

    auto *container = new QWidget(parent);
    container->setAttribute(Qt::WA_TransparentForMouseEvents);
    container->setContentsMargins(0, 0, 0, 0);
    auto *row = new QHBoxLayout(container);
    row->setContentsMargins(8, 0, 0, 0);
    row->setSpacing(3);

    if (shortcut.isEmpty()) {
        auto *empty = new QLabel(QStringLiteral("—"), container);
        empty->setStyleSheet("QLabel { color: #555; font-size: 12px; background: none; }");
        row->addWidget(empty);
    } else {
        // Split "Cmd+Shift+S" → ["Cmd","Shift","S"] and render each token
        QStringList tokens = shortcut.split('+', Qt::SkipEmptyParts);
        for (const QString &tok : tokens) {
            QString display = tok.trimmed();
            // Map common names to symbols
            if (display.compare("cmd", Qt::CaseInsensitive) == 0 ||
                display.compare("command", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌘");
            else if (display.compare("shift", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⇧");
            else if (display.compare("alt", Qt::CaseInsensitive) == 0 ||
                     display.compare("opt", Qt::CaseInsensitive) == 0 ||
                     display.compare("option", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌥");
            else if (display.compare("ctrl", Qt::CaseInsensitive) == 0 ||
                     display.compare("control", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌃");
            auto *badge = new QLabel(display, container);
            badge->setStyleSheet(badgeStyle);
            row->addWidget(badge);
        }
    }
    return container;
}

// Section heading helper
static QLabel *makeSectionLabel(const QString &text, QWidget *parent) {
    auto *lbl = new QLabel(text, parent);
    lbl->setStyleSheet(QStringLiteral(
        "QLabel { font-size: 11px; font-weight: 600; color: %1;"
        "  text-transform: uppercase; letter-spacing: 1px;"
        "  font-family: 'JetBrains Mono', monospace;"
        "  margin-top: 8px; margin-bottom: 4px; background: transparent; }"
    ).arg(g_theme.textMuted));
    return lbl;
}

// Thin separator line
static QFrame *makeSeparator(QWidget *parent) {
    auto *line = new QFrame(parent);
    line->setFrameShape(QFrame::HLine);
    line->setStyleSheet(QStringLiteral("QFrame { background: %1; border: none; max-height: 1px; }").arg(g_theme.borderSubtle));
    return line;
}

// ===========================================================================
// Construction
// ===========================================================================

PreferencesWindow::PreferencesWindow(QWidget *parent)
    : QWidget(parent)
{
    setWindowTitle(QStringLiteral("Snapforge Preferences"));
    resize(620, 560);
    setStyleSheet(windowStyle());
    buildUi();
}

// ===========================================================================
// buildUi
// ===========================================================================

void PreferencesWindow::buildUi()
{
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(0, 0, 0, 0);
    root->setSpacing(0);

    // Tab widget
    auto *tabs = new QTabWidget(this);
    tabs->setStyleSheet(tabStyle());
    tabs->addTab(buildGeneralTab(),     QStringLiteral("General"));
    tabs->addTab(buildScreenshotsTab(), QStringLiteral("Screenshots"));
    tabs->addTab(buildRecordingTab(),   QStringLiteral("Recording"));
    tabs->addTab(buildHotkeysTab(),     QStringLiteral("Hotkeys"));
    root->addWidget(tabs, 1);

    // Dark separator
    root->addWidget(makeSeparator(this));

    // Bottom bar
    auto *bar = new QHBoxLayout;
    bar->setContentsMargins(16, 8, 16, 10);
    bar->setSpacing(12);

    m_statusLabel = new QLabel(this);
    m_statusLabel->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
    m_statusLabel->setStyleSheet("QLabel { color: #22c55e; font-size: 12px; font-family: 'JetBrains Mono', monospace; background: transparent; }");

    auto *saveBtn = new QPushButton(QStringLiteral("Save Settings"), this);
    saveBtn->setStyleSheet(saveBtnStyle());
    saveBtn->setDefault(true);

    bar->addWidget(m_statusLabel, 1);
    bar->addWidget(saveBtn);
    root->addLayout(bar);

    connect(saveBtn, &QPushButton::clicked, this, &PreferencesWindow::onSave);
}

// ===========================================================================
// General tab
// ===========================================================================

QWidget *PreferencesWindow::buildGeneralTab()
{
    auto *w = new QWidget;
    w->setStyleSheet("QWidget { background: " + g_theme.bg + "; }");
    auto *form = new QVBoxLayout(w);
    form->setContentsMargins(24, 24, 24, 24);
    form->setSpacing(16);

    // — Save directory —
    form->addWidget(makeSectionLabel(QStringLiteral("Save Location"), w));

    auto *dirRow = new QHBoxLayout;
    dirRow->setSpacing(8);
    m_saveDir = new QLineEdit(w);
    m_saveDir->setPlaceholderText(QStringLiteral("~/Pictures/Snapforge"));
    m_saveDir->setStyleSheet(lineEditStyle());

    auto *browseBtn = new QPushButton(QStringLiteral("Browse..."), w);
    browseBtn->setStyleSheet(defaultBtnStyle());
    browseBtn->setFixedWidth(90);

    dirRow->addWidget(m_saveDir, 1);
    dirRow->addWidget(browseBtn);
    form->addLayout(dirRow);

    // — Appearance —
    form->addWidget(makeSectionLabel(QStringLiteral("Appearance"), w));

    auto *themeRow = new QHBoxLayout;
    themeRow->setSpacing(8);
    auto *themeLbl = new QLabel(QStringLiteral("Theme"), w);
    themeLbl->setStyleSheet(QStringLiteral("QLabel { color: %1; font-size: 14px; background: transparent; }").arg(g_theme.text));
    m_themeCombo = new QComboBox(w);
    m_themeCombo->addItems({"Auto", "Light", "Dark"});
    m_themeCombo->setStyleSheet(
        QStringLiteral(
            "QComboBox { background: %1; border: 1px solid %2; border-radius: 6px;"
            "  padding: 6px 12px; color: %3; font-size: 13px; }"
            "QComboBox:hover { border-color: %4; }"
            "QComboBox::drop-down { border: none; }"
            "QComboBox QAbstractItemView { background: %5; border: 1px solid %2;"
            "  color: %3; selection-background-color: %6; }"
        ).arg(g_theme.bgInput, g_theme.border, g_theme.text,
              g_theme.borderStrong, g_theme.bgCard, g_theme.bgActive)
    );
    m_themeCombo->setFixedWidth(120);
    themeRow->addWidget(themeLbl);
    themeRow->addStretch();
    themeRow->addWidget(m_themeCombo);
    form->addLayout(themeRow);

    connect(m_themeCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, [this](int) { applyTheme(); });

    form->addWidget(makeSeparator(w));

    // — Behaviour —
    form->addWidget(makeSectionLabel(QStringLiteral("Behaviour"), w));

    const QString cbStyle = checkBoxStyle();

    m_autoCopy = new QCheckBox(QStringLiteral("Auto copy to clipboard"), w);
    m_autoCopy->setStyleSheet(cbStyle);

    m_showNotif = new QCheckBox(QStringLiteral("Show notification"), w);
    m_showNotif->setStyleSheet(cbStyle);

    m_rememberRegion = new QCheckBox(QStringLiteral("Remember last region"), w);
    m_rememberRegion->setStyleSheet(cbStyle);

    form->addWidget(m_autoCopy);
    form->addWidget(m_showNotif);
    form->addWidget(m_rememberRegion);

    form->addStretch();

    connect(browseBtn, &QPushButton::clicked, this, &PreferencesWindow::onBrowseDirectory);

    return w;
}

// ===========================================================================
// Screenshots tab
// ===========================================================================

QWidget *PreferencesWindow::buildScreenshotsTab()
{
    auto *w = new QWidget;
    w->setStyleSheet("QWidget { background: " + g_theme.bg + "; }");
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(24, 24, 24, 24);
    layout->setSpacing(16);

    // — Format —
    layout->addWidget(makeSectionLabel(QStringLiteral("Format"), w));

    auto *fmtRow = new QHBoxLayout;
    fmtRow->setSpacing(8);

    m_screenshotFmtGroup = new QButtonGroup(w);
    m_screenshotFmtGroup->setExclusive(true);

    m_fmtPngBtn  = new QPushButton(QStringLiteral("PNG"),  w);
    m_fmtJpgBtn  = new QPushButton(QStringLiteral("JPG"),  w);
    m_fmtWebpBtn = new QPushButton(QStringLiteral("WebP"), w);

    for (auto *btn : {m_fmtPngBtn, m_fmtJpgBtn, m_fmtWebpBtn}) {
        btn->setCheckable(true);
        btn->setFixedWidth(90);
        fmtRow->addWidget(btn);
    }
    m_fmtPngBtn->setChecked(true);
    fmtRow->addStretch();
    layout->addLayout(fmtRow);

    m_screenshotFmtGroup->addButton(m_fmtPngBtn,  0);
    m_screenshotFmtGroup->addButton(m_fmtJpgBtn,  1);
    m_screenshotFmtGroup->addButton(m_fmtWebpBtn, 2);

    // Update styles when selection changes
    auto updateFmtStyles = [this]() {
        m_fmtPngBtn->setStyleSheet(cardStyle(m_fmtPngBtn->isChecked()));
        m_fmtJpgBtn->setStyleSheet(cardStyle(m_fmtJpgBtn->isChecked()));
        m_fmtWebpBtn->setStyleSheet(cardStyle(m_fmtWebpBtn->isChecked()));
        onScreenshotFormatChanged();
    };
    // Initial style
    m_fmtPngBtn->setStyleSheet(cardStyle(true));
    m_fmtJpgBtn->setStyleSheet(cardStyle(false));
    m_fmtWebpBtn->setStyleSheet(cardStyle(false));

    connect(m_screenshotFmtGroup, &QButtonGroup::idClicked, this, [updateFmtStyles](int) {
        updateFmtStyles();
    });

    // — Quality —
    layout->addWidget(makeSectionLabel(QStringLiteral("Quality"), w));

    auto *qualRow = new QHBoxLayout;
    qualRow->setSpacing(12);
    m_quality = new QSlider(Qt::Horizontal, w);
    m_quality->setRange(1, 100);
    m_quality->setValue(90);
    m_quality->setStyleSheet(sliderStyle());
    m_qualityLabel = new QLabel(QStringLiteral("90"), w);
    m_qualityLabel->setFixedWidth(30);
    m_qualityLabel->setStyleSheet("QLabel { color: #8b95ab; font-family: 'JetBrains Mono', monospace; background: transparent; }");
    qualRow->addWidget(m_quality, 1);
    qualRow->addWidget(m_qualityLabel);
    layout->addLayout(qualRow);

    connect(m_quality, &QSlider::valueChanged, this, [this](int v) {
        m_qualityLabel->setText(QString::number(v));
    });

    // — Filename pattern —
    layout->addWidget(makeSectionLabel(QStringLiteral("Filename Pattern"), w));

    m_filenamePattern = new QLineEdit(w);
    m_filenamePattern->setPlaceholderText(QStringLiteral("screenshot-{date}-{time}"));
    m_filenamePattern->setStyleSheet(lineEditStyle());
    layout->addWidget(m_filenamePattern);

    layout->addStretch();
    onScreenshotFormatChanged();
    return w;
}

// ===========================================================================
// Recording tab
// ===========================================================================

QWidget *PreferencesWindow::buildRecordingTab()
{
    auto *w = new QWidget;
    w->setStyleSheet("QWidget { background: " + g_theme.bg + "; }");
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(24, 24, 24, 24);
    layout->setSpacing(16);

    // — Format —
    layout->addWidget(makeSectionLabel(QStringLiteral("Format"), w));

    auto *fmtRow = new QHBoxLayout;
    fmtRow->setSpacing(8);

    m_recFmtGroup = new QButtonGroup(w);
    m_recFmtGroup->setExclusive(true);

    m_recMp4Btn = new QPushButton(QStringLiteral("MP4"), w);
    m_recGifBtn = new QPushButton(QStringLiteral("GIF"), w);

    for (auto *btn : {m_recMp4Btn, m_recGifBtn}) {
        btn->setCheckable(true);
        btn->setFixedWidth(90);
        fmtRow->addWidget(btn);
    }
    m_recMp4Btn->setChecked(true);
    fmtRow->addStretch();
    layout->addLayout(fmtRow);

    m_recFmtGroup->addButton(m_recMp4Btn, 0);
    m_recFmtGroup->addButton(m_recGifBtn, 1);

    auto updateRecFmtStyles = [this]() {
        m_recMp4Btn->setStyleSheet(cardStyle(m_recMp4Btn->isChecked()));
        m_recGifBtn->setStyleSheet(cardStyle(m_recGifBtn->isChecked()));
    };
    m_recMp4Btn->setStyleSheet(cardStyle(true));
    m_recGifBtn->setStyleSheet(cardStyle(false));
    connect(m_recFmtGroup, &QButtonGroup::idClicked, this, [updateRecFmtStyles](int) {
        updateRecFmtStyles();
    });

    // — FPS —
    layout->addWidget(makeSectionLabel(QStringLiteral("Frame Rate"), w));

    auto *fpsRow = new QHBoxLayout;
    fpsRow->setSpacing(6);
    m_fpsGroup = new QButtonGroup(w);
    m_fpsGroup->setExclusive(true);

    const int fpsValues[] = {10, 15, 24, 30, 60};
    for (int fps : fpsValues) {
        auto *btn = new QPushButton(QString::number(fps) + QStringLiteral(" fps"), w);
        btn->setCheckable(true);
        if (fps == 30) btn->setChecked(true);
        m_fpsGroup->addButton(btn, fps);
        fpsRow->addWidget(btn);
    }
    fpsRow->addStretch();
    layout->addLayout(fpsRow);

    // Style all fps pills
    auto updateFpsPills = [this]() {
        for (QAbstractButton *btn : m_fpsGroup->buttons())
            btn->setStyleSheet(pillStyle(btn->isChecked()));
    };
    updateFpsPills();
    connect(m_fpsGroup, &QButtonGroup::idClicked, this, [updateFpsPills](int) {
        updateFpsPills();
    });

    // — Quality —
    layout->addWidget(makeSectionLabel(QStringLiteral("Quality"), w));

    auto *qualRow = new QHBoxLayout;
    qualRow->setSpacing(6);
    m_recQualGroup = new QButtonGroup(w);
    m_recQualGroup->setExclusive(true);

    const char *qualNames[] = {"Low", "Medium", "High"};
    for (int i = 0; i < 3; ++i) {
        auto *btn = new QPushButton(QString::fromLatin1(qualNames[i]), w);
        btn->setCheckable(true);
        if (i == 1) btn->setChecked(true);
        m_recQualGroup->addButton(btn, i);
        qualRow->addWidget(btn);
    }
    qualRow->addStretch();
    layout->addLayout(qualRow);

    auto updateQualPills = [this]() {
        for (QAbstractButton *btn : m_recQualGroup->buttons())
            btn->setStyleSheet(pillStyle(btn->isChecked()));
    };
    updateQualPills();
    connect(m_recQualGroup, &QButtonGroup::idClicked, this, [updateQualPills](int) {
        updateQualPills();
    });

    layout->addStretch();
    return w;
}

// ===========================================================================
// Hotkeys tab
// ===========================================================================

QWidget *PreferencesWindow::buildHotkeysTab()
{
    // Outer scroll area
    auto *scroll = new QScrollArea;
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setStyleSheet("QScrollArea { background: #0c0e12; border: none; }"
                          "QScrollBar:vertical { background: #13161c; width: 8px; }"
                          "QScrollBar::handle:vertical { background: #3a4460; border-radius: 4px; min-height: 20px; }"
                          "QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }");

    auto *w = new QWidget;
    w->setStyleSheet("QWidget { background: " + g_theme.bg + "; }");
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(20, 20, 20, 20);
    layout->setSpacing(0);

    // Default hotkey table: section, actionKey, displayName, defaultShortcut
    struct HotkeyDef {
        const char *section;
        const char *actionKey;
        const char *displayName;
        const char *defaultShortcut;
    };
    static const HotkeyDef defs[] = {
        // Global
        {"global", "screenshot",  "Screenshot",   "Cmd+Shift+S"},
        {"global", "record",      "Record",        "Cmd+Shift+R"},
        {"global", "history",     "History",       "Cmd+Shift+H"},
        {"global", "preferences", "Preferences",   "Cmd+,"},
        // Tools
        {"tools", "arrow",        "Arrow",          "A"},
        {"tools", "rect",         "Rectangle",      "R"},
        {"tools", "circle",       "Circle",         "C"},
        {"tools", "line",         "Line",           "L"},
        {"tools", "dottedline",   "Dotted Line",    "D"},
        {"tools", "freehand",     "Freehand",       "F"},
        {"tools", "text",         "Text",           "T"},
        {"tools", "highlight",    "Highlight",      "H"},
        {"tools", "blur",         "Blur",           "B"},
        {"tools", "steps",        "Steps",          "N"},
        {"tools", "colorpicker",  "Color Picker",   "I"},
        {"tools", "measure",      "Measure",        "M"},
        // Sizes
        {"sizes", "small",        "Small",          "1"},
        {"sizes", "medium",       "Medium",         "2"},
        {"sizes", "large",        "Large",          "3"},
        // Actions
        {"actions", "save",       "Save",           "Cmd+S"},
        {"actions", "copy",       "Copy",           "Cmd+C"},
        {"actions", "undo",       "Undo",           "Cmd+Z"},
        {"actions", "redo",       "Redo",           "Cmd+Shift+Z"},
        {"actions", "cancel",     "Cancel",         "Escape"},
    };

    static const struct { const char *key; const char *label; } sectionLabels[] = {
        {"global",  "GLOBAL SHORTCUTS"},
        {"tools",   "ANNOTATION TOOLS"},
        {"sizes",   "STROKE SIZE"},
        {"actions", "OVERLAY ACTIONS"},
    };

    m_hotkeyRows.clear();
    const char *lastSection = nullptr;

    for (const HotkeyDef &def : defs) {
        // Section heading
        if (lastSection == nullptr || strcmp(def.section, lastSection) != 0) {
            lastSection = def.section;
            if (m_hotkeyRows.size() > 0) {
                layout->addSpacing(20);
            }
            for (const auto &sl : sectionLabels) {
                if (strcmp(sl.key, def.section) == 0) {
                    auto *sectionLbl = new QLabel(QString::fromLatin1(sl.label), w);
                    sectionLbl->setStyleSheet(
                        "QLabel { color: #3d4660; font-size: 10px; font-weight: 700;"
                        "  letter-spacing: 1.5px; background: transparent;"
                        "  font-family: 'JetBrains Mono', Menlo, monospace;"
                        "  padding-bottom: 6px; }");
                    layout->addWidget(sectionLbl);
                    // Column header
                    auto *hdr = new QWidget(w);
                    hdr->setFixedHeight(28);
                    hdr->setStyleSheet("QWidget { background: transparent; }");
                    auto *hdrLayout = new QHBoxLayout(hdr);
                    hdrLayout->setContentsMargins(14, 0, 14, 0);
                    hdrLayout->setSpacing(8);
                    auto *hAction = new QLabel(
                        strcmp(def.section, "tools") == 0 ? QStringLiteral("Tool") :
                        strcmp(def.section, "sizes") == 0 ? QStringLiteral("Size") :
                        QStringLiteral("Action"), hdr);
                    hAction->setStyleSheet("QLabel { color: #3d4660; font-size: 10px;"
                        " font-family: 'JetBrains Mono', monospace; background: transparent; }");
                    hAction->setFixedWidth(150);
                    auto *hShortcut = new QLabel(QStringLiteral("Shortcut"), hdr);
                    hShortcut->setStyleSheet("QLabel { color: #3d4660; font-size: 10px;"
                        " font-family: 'JetBrains Mono', monospace; background: transparent; }");
                    hdrLayout->addWidget(hAction);
                    hdrLayout->addWidget(hShortcut, 1);
                    hdrLayout->addSpacing(80); // space for Change button column
                    layout->addWidget(hdr);
                    // Separator under header
                    auto *sep = new QFrame(w);
                    sep->setFrameShape(QFrame::HLine);
                    sep->setStyleSheet("QFrame { background: #262d3d; border: none; max-height: 1px; }");
                    layout->addWidget(sep);
                    break;
                }
            }
        }

        HotkeyRow row;
        row.section     = QString::fromLatin1(def.section);
        row.actionKey   = QString::fromLatin1(def.actionKey);
        row.displayName = QString::fromLatin1(def.displayName);
        row.shortcut    = QString::fromLatin1(def.defaultShortcut);
        row.recording   = false;

        // Each row is a flat widget with a bottom border
        auto *rowWidget = new QWidget(w);
        rowWidget->setStyleSheet(
            "QWidget { background: transparent; }");
        rowWidget->setFixedHeight(40);

        auto *rowLayout = new QHBoxLayout(rowWidget);
        rowLayout->setContentsMargins(14, 0, 14, 0);
        rowLayout->setSpacing(8);

        // Action label
        auto *nameLabel = new QLabel(row.displayName, rowWidget);
        nameLabel->setStyleSheet(
            "QLabel { color: #e8ecf4; font-size: 13px; font-weight: 450;"
            "  background: transparent; }");
        nameLabel->setFixedWidth(150);

        // Badge widget (shortcut display)
        row.badgeWidget = makeBadges(row.shortcut, rowWidget);

        // Change button
        int rowIndex = m_hotkeyRows.size();
        auto *changeBtn = new QPushButton(QStringLiteral("Change"), rowWidget);
        changeBtn->setStyleSheet(
            "QPushButton { background: transparent; border: 1px solid #262d3d;"
            "  border-radius: 4px; padding: 3px 12px; color: #5a6580; font-size: 10px;"
            "  font-family: 'JetBrains Mono', monospace; }"
            "QPushButton:hover { color: #60a5fa; border-color: #3b82f6;"
            "  background: rgba(59,130,246,0.08); }");
        changeBtn->setFixedWidth(70);
        row.changeBtn = changeBtn;

        rowLayout->addWidget(nameLabel);
        rowLayout->addWidget(row.badgeWidget);
        rowLayout->addStretch(1);
        rowLayout->addWidget(changeBtn);

        layout->addWidget(rowWidget);

        // Thin separator between rows
        auto *rowSep = new QFrame(w);
        rowSep->setFrameShape(QFrame::HLine);
        rowSep->setStyleSheet("QFrame { background: #1e2330; border: none; max-height: 1px; }");
        layout->addWidget(rowSep);

        m_hotkeyRows.append(row);

        connect(changeBtn, &QPushButton::clicked, this, [this, rowIndex]() {
            if (m_recordingRowIndex == rowIndex) {
                stopRecording(rowIndex, false);
            } else {
                startRecording(rowIndex);
            }
        });
    }

    layout->addStretch();

    // Reset button — left aligned
    layout->addSpacing(16);
    auto *resetBtn = new QPushButton(QStringLiteral("Reset All to Defaults"), w);
    resetBtn->setStyleSheet(resetBtnStyle());

    auto *btnRow = new QHBoxLayout;
    btnRow->addWidget(resetBtn);
    btnRow->addStretch();
    layout->addLayout(btnRow);

    connect(resetBtn, &QPushButton::clicked, this, &PreferencesWindow::onResetHotkeys);

    scroll->setWidget(w);
    scroll->installEventFilter(this);

    return scroll;
}

// ===========================================================================
// Event overrides
// ===========================================================================

void PreferencesWindow::showEvent(QShowEvent *event)
{
    QWidget::showEvent(event);
    loadConfig();
}

void PreferencesWindow::hideEvent(QHideEvent *event)
{
    // Make sure we don't leak a keyboard grab if the window is hidden while
    // the user was mid-recording a hotkey.
    if (m_recordingRowIndex >= 0) {
        stopRecording(m_recordingRowIndex, false);
    }
    QWidget::hideEvent(event);
}

void PreferencesWindow::closeEvent(QCloseEvent *event)
{
    if (m_recordingRowIndex >= 0) {
        stopRecording(m_recordingRowIndex, false);
    }
    QWidget::closeEvent(event);
}

bool PreferencesWindow::eventFilter(QObject *obj, QEvent *event)
{
    if (event->type() == QEvent::KeyPress && m_recordingRowIndex >= 0) {
        auto *ke = static_cast<QKeyEvent *>(event);
        int key = ke->key();

        if (key == Qt::Key_Escape) {
            stopRecording(m_recordingRowIndex, false);
            return true;
        }
        // Ignore lone modifier presses
        if (key == Qt::Key_Control || key == Qt::Key_Shift ||
            key == Qt::Key_Alt    || key == Qt::Key_Meta) {
            return false;
        }
        QString newShortcut = formatKeyToShortcut(key, ke->modifiers());
        stopRecording(m_recordingRowIndex, true, newShortcut);
        return true;
    }
    return QWidget::eventFilter(obj, event);
}

// ===========================================================================
// Hotkey recording helpers
// ===========================================================================

void PreferencesWindow::startRecording(int rowIndex)
{
    // Cancel any existing recording
    if (m_recordingRowIndex >= 0 && m_recordingRowIndex != rowIndex) {
        stopRecording(m_recordingRowIndex, false);
    }

    m_recordingRowIndex = rowIndex;
    HotkeyRow &row = m_hotkeyRows[rowIndex];
    row.recording = true;

    row.changeBtn->setText(QStringLiteral("Press a key..."));
    row.changeBtn->setStyleSheet(
        "QPushButton { background: rgba(59,130,246,0.15); border: 1px solid #3b82f6;"
        "  border-radius: 5px; padding: 4px 12px; color: #60a5fa; font-size: 12px;"
        "  font-family: 'JetBrains Mono', monospace; }");
    row.changeBtn->setFixedWidth(110);

    // Grab keyboard focus
    grabKeyboard();
    installEventFilter(this);
}

void PreferencesWindow::stopRecording(int rowIndex, bool accept, const QString &newShortcut)
{
    if (rowIndex < 0 || rowIndex >= m_hotkeyRows.size()) return;

    HotkeyRow &row = m_hotkeyRows[rowIndex];
    row.recording = false;

    if (accept && !newShortcut.isEmpty()) {
        row.shortcut = newShortcut;
        refreshBadge(rowIndex);
    }

    row.changeBtn->setText(QStringLiteral("Change"));
    row.changeBtn->setStyleSheet(
        "QPushButton { background: #151920; border: 1px solid #262d3d;"
        "  border-radius: 5px; padding: 4px 12px; color: #5a6580; font-size: 12px;"
        "  font-family: 'JetBrains Mono', monospace; }"
        "QPushButton:hover { color: #60a5fa; border-color: #3b82f6; background: rgba(59,130,246,0.1); }");
    row.changeBtn->setFixedWidth(80);

    m_recordingRowIndex = -1;
    releaseKeyboard();
}

void PreferencesWindow::refreshBadge(int rowIndex)
{
    HotkeyRow &row = m_hotkeyRows[rowIndex];
    if (!row.badgeWidget) return;

    // Replace badge widget contents (delete old children, rebuild)
    QLayoutItem *item;
    while ((item = row.badgeWidget->layout()->takeAt(0)) != nullptr) {
        if (item->widget()) item->widget()->deleteLater();
        delete item;
    }

    // Rebuild using makeBadges logic inline on existing container
    static const QString badgeStyle =
        "QLabel { background: #1e2330; border: 1px solid #262d3d; border-radius: 3px; padding: 1px 5px;"
        "  font-size: 10px; font-family: 'JetBrains Mono', Menlo, monospace; color: #8b95ab; }";

    auto *rowLayout = qobject_cast<QHBoxLayout *>(row.badgeWidget->layout());
    if (!rowLayout) return;

    if (row.shortcut.isEmpty()) {
        auto *empty = new QLabel(QStringLiteral("—"), row.badgeWidget);
        empty->setStyleSheet("QLabel { color: #3d4660; font-size: 12px; background: none; }");
        rowLayout->addWidget(empty);
    } else {
        QStringList tokens = row.shortcut.split('+', Qt::SkipEmptyParts);
        for (const QString &tok : tokens) {
            QString display = tok.trimmed();
            if (display.compare("cmd", Qt::CaseInsensitive) == 0 ||
                display.compare("command", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌘");
            else if (display.compare("shift", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⇧");
            else if (display.compare("alt", Qt::CaseInsensitive) == 0 ||
                     display.compare("opt", Qt::CaseInsensitive) == 0 ||
                     display.compare("option", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌥");
            else if (display.compare("ctrl", Qt::CaseInsensitive) == 0 ||
                     display.compare("control", Qt::CaseInsensitive) == 0)
                display = QStringLiteral("⌃");
            auto *badge = new QLabel(display, row.badgeWidget);
            badge->setStyleSheet(badgeStyle);
            rowLayout->addWidget(badge);
        }
    }
}

// Convert Qt key + modifiers to a shortcut string like "Cmd+Shift+S"
QString PreferencesWindow::formatKeyToShortcut(int key, Qt::KeyboardModifiers mods)
{
    QStringList parts;
    if (mods & Qt::ControlModifier) parts << QStringLiteral("Cmd");
    if (mods & Qt::ShiftModifier)   parts << QStringLiteral("Shift");
    if (mods & Qt::AltModifier)     parts << QStringLiteral("Alt");

    // Special key names
    switch (key) {
    case Qt::Key_Escape:    parts << QStringLiteral("Escape"); break;
    case Qt::Key_Return:    parts << QStringLiteral("Enter");  break;
    case Qt::Key_Enter:     parts << QStringLiteral("Enter");  break;
    case Qt::Key_Backspace: parts << QStringLiteral("Backspace"); break;
    case Qt::Key_Delete:    parts << QStringLiteral("Delete"); break;
    case Qt::Key_Tab:       parts << QStringLiteral("Tab");    break;
    case Qt::Key_Space:     parts << QStringLiteral("Space");  break;
    case Qt::Key_Comma:     parts << QStringLiteral(",");       break;
    default: {
        QString ks = QKeySequence(key).toString();
        if (!ks.isEmpty()) parts << ks;
        break;
    }
    }
    return parts.join('+');
}

// ===========================================================================
// Slots
// ===========================================================================

void PreferencesWindow::onBrowseDirectory()
{
    QString dir = QFileDialog::getExistingDirectory(
        this,
        QStringLiteral("Select Save Directory"),
        m_saveDir->text().isEmpty() ? QDir::homePath() : m_saveDir->text());
    if (!dir.isEmpty()) {
        m_saveDir->setText(dir);
    }
}

void PreferencesWindow::onScreenshotFormatChanged()
{
    bool needsQuality = (m_screenshotFmtGroup &&
                         m_screenshotFmtGroup->checkedId() != 0); // 0 = PNG
    if (m_quality) m_quality->setEnabled(needsQuality);
    if (m_qualityLabel) m_qualityLabel->setEnabled(needsQuality);
}

void PreferencesWindow::onResetHotkeys()
{
    static const struct { const char *section; const char *key; const char *shortcut; } defaults[] = {
        {"global", "screenshot",  "Cmd+Shift+S"},
        {"global", "record",      "Cmd+Shift+R"},
        {"global", "history",     "Cmd+Shift+H"},
        {"global", "preferences", "Cmd+,"},
        {"tools",  "arrow",       "A"},
        {"tools",  "rect",        "R"},
        {"tools",  "circle",      "C"},
        {"tools",  "line",        "L"},
        {"tools",  "dottedline",  "D"},
        {"tools",  "freehand",    "F"},
        {"tools",  "text",        "T"},
        {"tools",  "highlight",   "H"},
        {"tools",  "blur",        "B"},
        {"tools",  "steps",       "N"},
        {"tools",  "colorpicker", "I"},
        {"tools",  "measure",     "M"},
        {"sizes",  "small",       "1"},
        {"sizes",  "medium",      "2"},
        {"sizes",  "large",       "3"},
        {"actions","save",        "Cmd+S"},
        {"actions","copy",        "Cmd+C"},
        {"actions","undo",        "Cmd+Z"},
        {"actions","redo",        "Cmd+Shift+Z"},
        {"actions","cancel",      "Escape"},
    };

    for (int i = 0; i < (int)m_hotkeyRows.size(); ++i) {
        HotkeyRow &row = m_hotkeyRows[i];
        for (const auto &d : defaults) {
            if (row.section == QString::fromLatin1(d.section) &&
                row.actionKey == QString::fromLatin1(d.key)) {
                row.shortcut = QString::fromLatin1(d.shortcut);
                refreshBadge(i);
                break;
            }
        }
    }
    m_statusLabel->setText(QStringLiteral("Hotkeys reset to defaults."));
    QTimer::singleShot(3000, this, [this]() { m_statusLabel->clear(); });
}

void PreferencesWindow::onSave()
{
    // Fix #15: block duplicate hotkey bindings from being persisted. Two
    // actions on the same chord is always a bug — one of them will never
    // fire. Single pass: bucket every non-empty shortcut; any bucket with
    // more than one entry is a conflict. Dedupe the reported list so a
    // shortcut bound three times doesn't show three copies of each row.
    QMap<QString, QStringList> buckets; // shortcut -> list of "section: action"
    for (const HotkeyRow &row : m_hotkeyRows) {
        if (row.shortcut.isEmpty()) continue;
        buckets[row.shortcut]
            << (row.section + QStringLiteral(": ") + row.displayName);
    }
    QStringList dupes;
    for (auto it = buckets.constBegin(); it != buckets.constEnd(); ++it) {
        if (it.value().size() > 1) {
            dupes << (it.key() + QStringLiteral("  →  ")
                      + it.value().join(QStringLiteral(", ")));
        }
    }
    if (!dupes.isEmpty()) {
        QMessageBox::warning(this,
            QStringLiteral("Duplicate shortcuts"),
            QStringLiteral("The following shortcuts are bound to more than one action; "
                           "change them before saving:\n\n") + dupes.join('\n'));
        return;
    }
    saveConfig();
    emit configSaved();
}

// ===========================================================================
// Theme
// ===========================================================================

void PreferencesWindow::applyTheme()
{
    if (!m_themeCombo) return;

    int idx = m_themeCombo->currentIndex();
    if (idx == 0) {
        // Auto — detect system appearance
#ifdef Q_OS_MAC
        // Check macOS dark mode
        QPalette pal = QApplication::palette();
        bool sysDark = pal.color(QPalette::Window).lightness() < 128;
        g_theme = sysDark ? darkTheme() : lightTheme();
#else
        g_theme = darkTheme();
#endif
    } else if (idx == 1) {
        g_theme = lightTheme();
    } else {
        g_theme = darkTheme();
    }

    // Re-apply the window stylesheet — this cascades to all children
    setStyleSheet(windowStyle());

    // M14: instead of hiding/showing (which visibly flickers + moves the
    // window), force the style engine to re-evaluate rules on the root
    // widget. This re-polishes every child automatically.
    if (QStyle *s = style()) {
        s->unpolish(this);
        s->polish(this);
    }
    update();
}

// ===========================================================================
// Config load / save
// ===========================================================================

void PreferencesWindow::loadConfig()
{
    char *raw = snapforge_config_load();
    if (!raw) {
        m_statusLabel->setText(QStringLiteral("Failed to load config."));
        return;
    }

    QByteArray jsonBytes(raw);
    snapforge_free_string(raw);

    QJsonParseError err;
    QJsonDocument doc = QJsonDocument::fromJson(jsonBytes, &err);
    if (doc.isNull() || !doc.isObject()) {
        m_statusLabel->setText(QStringLiteral("Config parse error: ") + err.errorString());
        return;
    }

    QJsonObject obj = doc.object();

    // — General —
    m_saveDir->setText(obj.value(QStringLiteral("save_directory")).toString());
    m_autoCopy->setChecked(obj.value(QStringLiteral("auto_copy_clipboard")).toBool());
    m_showNotif->setChecked(obj.value(QStringLiteral("show_notification")).toBool());
    m_rememberRegion->setChecked(obj.value(QStringLiteral("remember_last_region")).toBool());

    // — Theme —
    QString themeName = obj.value(QStringLiteral("theme")).toString(QStringLiteral("Auto"));
    if (m_themeCombo) {
        int themeIdx = 0;
        if (themeName == QStringLiteral("Light")) themeIdx = 1;
        else if (themeName == QStringLiteral("Dark")) themeIdx = 2;
        m_themeCombo->blockSignals(true);
        m_themeCombo->setCurrentIndex(themeIdx);
        m_themeCombo->blockSignals(false);
        applyTheme();
    }

    // — Screenshots format —
    QString fmt = obj.value(QStringLiteral("screenshot_format")).toString();
    if (fmt == QStringLiteral("Jpg") || fmt == QStringLiteral("JPG")) {
        m_fmtJpgBtn->setChecked(true);
    } else if (fmt == QStringLiteral("WebP") || fmt == QStringLiteral("Webp")) {
        m_fmtWebpBtn->setChecked(true);
    } else {
        m_fmtPngBtn->setChecked(true);
    }
    m_fmtPngBtn->setStyleSheet(cardStyle(m_fmtPngBtn->isChecked()));
    m_fmtJpgBtn->setStyleSheet(cardStyle(m_fmtJpgBtn->isChecked()));
    m_fmtWebpBtn->setStyleSheet(cardStyle(m_fmtWebpBtn->isChecked()));

    // — Quality —
    int quality = obj.value(QStringLiteral("jpg_quality")).toInt(90);
    m_quality->setValue(quality);
    m_qualityLabel->setText(QString::number(quality));

    // — Filename pattern —
    m_filenamePattern->setText(obj.value(QStringLiteral("filename_pattern")).toString());

    // — Recording —
    QJsonObject rec = obj.value(QStringLiteral("recording")).toObject();
    QString recFmt = rec.value(QStringLiteral("format")).toString();
    if (recFmt == QStringLiteral("Gif") || recFmt == QStringLiteral("GIF")) {
        m_recGifBtn->setChecked(true);
    } else {
        m_recMp4Btn->setChecked(true);
    }
    m_recMp4Btn->setStyleSheet(cardStyle(m_recMp4Btn->isChecked()));
    m_recGifBtn->setStyleSheet(cardStyle(m_recGifBtn->isChecked()));

    int fps = rec.value(QStringLiteral("fps")).toInt(30);
    QAbstractButton *fpsBtn = m_fpsGroup->button(fps);
    if (fpsBtn) {
        fpsBtn->setChecked(true);
    } else {
        QAbstractButton *fb = m_fpsGroup->button(30);
        if (fb) fb->setChecked(true);
    }
    for (QAbstractButton *btn : m_fpsGroup->buttons())
        btn->setStyleSheet(pillStyle(btn->isChecked()));

    QString qual = rec.value(QStringLiteral("quality")).toString();
    int qualIdx = 1;
    if (qual == QStringLiteral("Low"))  qualIdx = 0;
    else if (qual == QStringLiteral("High")) qualIdx = 2;
    QAbstractButton *qualBtn = m_recQualGroup->button(qualIdx);
    if (qualBtn) qualBtn->setChecked(true);
    for (QAbstractButton *btn : m_recQualGroup->buttons())
        btn->setStyleSheet(pillStyle(btn->isChecked()));

    // — Hotkeys —
    QJsonObject hotkeys = obj.value(QStringLiteral("hotkeys")).toObject();
    for (int i = 0; i < m_hotkeyRows.size(); ++i) {
        HotkeyRow &row = m_hotkeyRows[i];
        QJsonObject section = hotkeys.value(row.section).toObject();
        QString stored = section.value(row.actionKey).toString();
        if (!stored.isEmpty()) {
            row.shortcut = stored;
            refreshBadge(i);
        }
    }

    onScreenshotFormatChanged();
    m_statusLabel->clear();
}

void PreferencesWindow::saveConfig()
{
    QJsonObject obj;

    // — General —
    obj[QStringLiteral("save_directory")]       = m_saveDir->text();
    obj[QStringLiteral("auto_copy_clipboard")]  = m_autoCopy->isChecked();
    obj[QStringLiteral("show_notification")]    = m_showNotif->isChecked();
    obj[QStringLiteral("remember_last_region")] = m_rememberRegion->isChecked();

    // — Theme —
    if (m_themeCombo) {
        const QStringList themes = {QStringLiteral("Auto"), QStringLiteral("Light"), QStringLiteral("Dark")};
        obj[QStringLiteral("theme")] = themes.value(m_themeCombo->currentIndex(), QStringLiteral("Auto"));
    }

    // — Screenshots —
    QString fmt = QStringLiteral("Png");
    if (m_fmtJpgBtn->isChecked())       fmt = QStringLiteral("Jpg");
    else if (m_fmtWebpBtn->isChecked()) fmt = QStringLiteral("WebP");
    obj[QStringLiteral("screenshot_format")] = fmt;
    obj[QStringLiteral("jpg_quality")]       = m_quality->value();
    obj[QStringLiteral("filename_pattern")]  = m_filenamePattern->text();

    // — Recording —
    QJsonObject rec;
    rec[QStringLiteral("format")]  = m_recGifBtn->isChecked()
                                         ? QStringLiteral("Gif")
                                         : QStringLiteral("Mp4");
    int fps = 30;
    QAbstractButton *checkedFps = m_fpsGroup->checkedButton();
    if (checkedFps) fps = m_fpsGroup->id(checkedFps);
    rec[QStringLiteral("fps")] = fps;

    QString qualStr = QStringLiteral("Medium");
    int qualId = m_recQualGroup->checkedId();
    if (qualId == 0)      qualStr = QStringLiteral("Low");
    else if (qualId == 2) qualStr = QStringLiteral("High");
    rec[QStringLiteral("quality")] = qualStr;
    obj[QStringLiteral("recording")] = rec;

    // — Hotkeys —
    QJsonObject hotkeys;
    for (const HotkeyRow &row : m_hotkeyRows) {
        QJsonObject section = hotkeys.value(row.section).toObject();
        section[row.actionKey] = row.shortcut;
        hotkeys[row.section] = section;
    }
    obj[QStringLiteral("hotkeys")] = hotkeys;

    QByteArray jsonBytes = QJsonDocument(obj).toJson(QJsonDocument::Compact);
    int result = snapforge_config_save(jsonBytes.constData());
    if (result == 0) {
        m_statusLabel->setText(QStringLiteral("Saved."));
        QTimer::singleShot(3000, this, [this]() { m_statusLabel->clear(); });
    } else {
        m_statusLabel->setText(QStringLiteral("Error: failed to save config."));
    }
}
