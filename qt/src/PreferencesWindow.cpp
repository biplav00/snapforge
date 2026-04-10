#include "PreferencesWindow.h"

#include <QFileDialog>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonDocument>
#include <QJsonObject>
#include <QShowEvent>
#include <QTabWidget>
#include <QTimer>
#include <QVBoxLayout>

#include "snapforge_ffi.h"

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

PreferencesWindow::PreferencesWindow(QWidget *parent)
    : QWidget(parent)
{
    setWindowTitle(QStringLiteral("Snapforge Preferences"));
    resize(600, 480);
    buildUi();
}

// ---------------------------------------------------------------------------
// UI construction helpers
// ---------------------------------------------------------------------------

void PreferencesWindow::buildUi()
{
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(12, 12, 12, 12);
    root->setSpacing(8);

    auto *tabs = new QTabWidget(this);
    tabs->addTab(buildGeneralTab(),     QStringLiteral("General"));
    tabs->addTab(buildScreenshotsTab(), QStringLiteral("Screenshots"));
    tabs->addTab(buildRecordingTab(),   QStringLiteral("Recording"));
    tabs->addTab(buildHotkeysTab(),     QStringLiteral("Hotkeys"));
    root->addWidget(tabs);

    // Bottom bar
    auto *bar = new QHBoxLayout;
    bar->setContentsMargins(0, 4, 0, 0);

    m_statusLabel = new QLabel(this);
    m_statusLabel->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);

    auto *saveBtn = new QPushButton(QStringLiteral("Save"), this);
    saveBtn->setDefault(true);
    saveBtn->setFixedWidth(100);

    bar->addWidget(m_statusLabel, 1);
    bar->addWidget(saveBtn);
    root->addLayout(bar);

    connect(saveBtn, &QPushButton::clicked, this, &PreferencesWindow::onSave);
}

QWidget *PreferencesWindow::buildGeneralTab()
{
    auto *w = new QWidget;
    auto *form = new QVBoxLayout(w);
    form->setContentsMargins(16, 16, 16, 16);
    form->setSpacing(12);

    // Save directory row
    auto *dirRow = new QHBoxLayout;
    auto *dirLabel = new QLabel(QStringLiteral("Save directory:"), w);
    m_saveDir = new QLineEdit(w);
    auto *browseBtn = new QPushButton(QStringLiteral("Browse..."), w);
    browseBtn->setFixedWidth(80);
    dirRow->addWidget(dirLabel);
    dirRow->addWidget(m_saveDir, 1);
    dirRow->addWidget(browseBtn);
    form->addLayout(dirRow);

    connect(browseBtn, &QPushButton::clicked, this, &PreferencesWindow::onBrowseDirectory);

    // Check boxes
    m_autoCopy       = new QCheckBox(QStringLiteral("Auto copy to clipboard"), w);
    m_showNotif      = new QCheckBox(QStringLiteral("Show notification"), w);
    m_rememberRegion = new QCheckBox(QStringLiteral("Remember last region"), w);

    form->addWidget(m_autoCopy);
    form->addWidget(m_showNotif);
    form->addWidget(m_rememberRegion);
    form->addStretch();

    return w;
}

QWidget *PreferencesWindow::buildScreenshotsTab()
{
    auto *w = new QWidget;
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(16, 16, 16, 16);
    layout->setSpacing(12);

    // Format group
    auto *fmtLabel = new QLabel(QStringLiteral("Format:"), w);
    layout->addWidget(fmtLabel);

    auto *fmtRow = new QHBoxLayout;
    m_fmtPng  = new QRadioButton(QStringLiteral("PNG"),  w);
    m_fmtJpg  = new QRadioButton(QStringLiteral("JPG"),  w);
    m_fmtWebp = new QRadioButton(QStringLiteral("WebP"), w);
    m_fmtPng->setChecked(true);
    fmtRow->addWidget(m_fmtPng);
    fmtRow->addWidget(m_fmtJpg);
    fmtRow->addWidget(m_fmtWebp);
    fmtRow->addStretch();
    layout->addLayout(fmtRow);

    connect(m_fmtPng,  &QRadioButton::toggled, this, &PreferencesWindow::onScreenshotFormatChanged);
    connect(m_fmtJpg,  &QRadioButton::toggled, this, &PreferencesWindow::onScreenshotFormatChanged);
    connect(m_fmtWebp, &QRadioButton::toggled, this, &PreferencesWindow::onScreenshotFormatChanged);

    // Quality slider
    auto *qualRow = new QHBoxLayout;
    auto *qualLabel = new QLabel(QStringLiteral("Quality:"), w);
    m_quality = new QSlider(Qt::Horizontal, w);
    m_quality->setRange(1, 100);
    m_quality->setValue(90);
    m_qualityLabel = new QLabel(QStringLiteral("90"), w);
    m_qualityLabel->setFixedWidth(30);
    qualRow->addWidget(qualLabel);
    qualRow->addWidget(m_quality, 1);
    qualRow->addWidget(m_qualityLabel);
    layout->addLayout(qualRow);

    connect(m_quality, &QSlider::valueChanged, this, [this](int v) {
        m_qualityLabel->setText(QString::number(v));
    });

    // Filename pattern
    auto *patRow = new QHBoxLayout;
    auto *patLabel = new QLabel(QStringLiteral("Filename pattern:"), w);
    m_filenamePattern = new QLineEdit(w);
    m_filenamePattern->setPlaceholderText(QStringLiteral("screenshot-{date}-{time}"));
    patRow->addWidget(patLabel);
    patRow->addWidget(m_filenamePattern, 1);
    layout->addLayout(patRow);

    layout->addStretch();

    // Initial state
    onScreenshotFormatChanged();

    return w;
}

QWidget *PreferencesWindow::buildRecordingTab()
{
    auto *w = new QWidget;
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(16, 16, 16, 16);
    layout->setSpacing(12);

    // Format group
    auto *fmtLabel = new QLabel(QStringLiteral("Format:"), w);
    layout->addWidget(fmtLabel);

    auto *fmtRow = new QHBoxLayout;
    m_recMp4 = new QRadioButton(QStringLiteral("MP4"), w);
    m_recGif = new QRadioButton(QStringLiteral("GIF"), w);
    m_recMp4->setChecked(true);
    fmtRow->addWidget(m_recMp4);
    fmtRow->addWidget(m_recGif);
    fmtRow->addStretch();
    layout->addLayout(fmtRow);

    // FPS group
    auto *fpsLabel = new QLabel(QStringLiteral("FPS:"), w);
    layout->addWidget(fpsLabel);

    auto *fpsRow = new QHBoxLayout;
    m_fpsGroup = new QButtonGroup(w);
    m_fpsGroup->setExclusive(true);

    const int fpsValues[] = {10, 15, 24, 30, 60};
    for (int fps : fpsValues) {
        auto *btn = new QPushButton(QString::number(fps), w);
        btn->setCheckable(true);
        btn->setFixedWidth(48);
        if (fps == 30) {
            btn->setChecked(true);
        }
        m_fpsGroup->addButton(btn, fps);
        fpsRow->addWidget(btn);
    }
    fpsRow->addStretch();
    layout->addLayout(fpsRow);

    // Quality group
    auto *qualLabel = new QLabel(QStringLiteral("Quality:"), w);
    layout->addWidget(qualLabel);

    auto *qualRow = new QHBoxLayout;
    m_recQualGroup = new QButtonGroup(w);
    m_recQualGroup->setExclusive(true);

    const char *qualNames[] = {"Low", "Medium", "High"};
    for (int i = 0; i < 3; ++i) {
        auto *btn = new QPushButton(QString::fromLatin1(qualNames[i]), w);
        btn->setCheckable(true);
        btn->setFixedWidth(72);
        if (i == 1) { // Medium default
            btn->setChecked(true);
        }
        m_recQualGroup->addButton(btn, i);
        qualRow->addWidget(btn);
    }
    qualRow->addStretch();
    layout->addLayout(qualRow);

    layout->addStretch();

    return w;
}

QWidget *PreferencesWindow::buildHotkeysTab()
{
    auto *w = new QWidget;
    auto *layout = new QVBoxLayout(w);
    layout->setContentsMargins(16, 16, 16, 16);
    layout->setSpacing(12);

    m_hotkeysTable = new QTableWidget(w);
    m_hotkeysTable->setColumnCount(2);
    m_hotkeysTable->setHorizontalHeaderLabels({QStringLiteral("Action"), QStringLiteral("Shortcut")});
    m_hotkeysTable->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    m_hotkeysTable->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);
    m_hotkeysTable->setEditTriggers(QAbstractItemView::NoEditTriggers);
    m_hotkeysTable->setSelectionMode(QAbstractItemView::SingleSelection);
    m_hotkeysTable->verticalHeader()->setVisible(false);

    // Populate default rows
    const struct { const char *action; const char *shortcut; } defaults[] = {
        {"Screenshot",   "Cmd+Shift+4"},
        {"Record",       "Cmd+Shift+5"},
        {"History",      "Cmd+Shift+H"},
        {"Preferences",  "Cmd+,"},
    };
    m_hotkeysTable->setRowCount(4);
    for (int i = 0; i < 4; ++i) {
        m_hotkeysTable->setItem(i, 0, new QTableWidgetItem(QString::fromLatin1(defaults[i].action)));
        m_hotkeysTable->setItem(i, 1, new QTableWidgetItem(QString::fromLatin1(defaults[i].shortcut)));
    }

    layout->addWidget(m_hotkeysTable, 1);

    auto *resetBtn = new QPushButton(QStringLiteral("Reset to Defaults"), w);
    resetBtn->setFixedWidth(140);

    auto *btnRow = new QHBoxLayout;
    btnRow->addStretch();
    btnRow->addWidget(resetBtn);
    layout->addLayout(btnRow);

    connect(resetBtn, &QPushButton::clicked, this, &PreferencesWindow::onResetHotkeys);

    return w;
}

// ---------------------------------------------------------------------------
// Event overrides
// ---------------------------------------------------------------------------

void PreferencesWindow::showEvent(QShowEvent *event)
{
    QWidget::showEvent(event);
    loadConfig();
}

// ---------------------------------------------------------------------------
// Slots
// ---------------------------------------------------------------------------

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
    bool needsQuality = m_fmtJpg->isChecked() || m_fmtWebp->isChecked();
    m_quality->setEnabled(needsQuality);
    m_qualityLabel->setEnabled(needsQuality);
}

void PreferencesWindow::onResetHotkeys()
{
    const struct { const char *action; const char *shortcut; } defaults[] = {
        {"Screenshot",   "Cmd+Shift+4"},
        {"Record",       "Cmd+Shift+5"},
        {"History",      "Cmd+Shift+H"},
        {"Preferences",  "Cmd+,"},
    };
    for (int i = 0; i < 4; ++i) {
        m_hotkeysTable->item(i, 1)->setText(QString::fromLatin1(defaults[i].shortcut));
    }
    m_statusLabel->setText(QStringLiteral("Hotkeys reset to defaults."));
}

void PreferencesWindow::onSave()
{
    saveConfig();
}

// ---------------------------------------------------------------------------
// Config load / save
// ---------------------------------------------------------------------------

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

    // General
    m_saveDir->setText(obj.value(QStringLiteral("save_directory")).toString());
    m_autoCopy->setChecked(obj.value(QStringLiteral("auto_copy_clipboard")).toBool());
    m_showNotif->setChecked(obj.value(QStringLiteral("show_notification")).toBool());
    m_rememberRegion->setChecked(obj.value(QStringLiteral("remember_last_region")).toBool());

    // Screenshots — format
    QString fmt = obj.value(QStringLiteral("screenshot_format")).toString();
    if (fmt == QStringLiteral("Jpg") || fmt == QStringLiteral("JPG")) {
        m_fmtJpg->setChecked(true);
    } else if (fmt == QStringLiteral("WebP") || fmt == QStringLiteral("Webp")) {
        m_fmtWebp->setChecked(true);
    } else {
        m_fmtPng->setChecked(true);
    }

    // Quality
    int quality = obj.value(QStringLiteral("jpg_quality")).toInt(90);
    m_quality->setValue(quality);
    m_qualityLabel->setText(QString::number(quality));

    // Filename pattern
    m_filenamePattern->setText(obj.value(QStringLiteral("filename_pattern")).toString());

    // Recording
    QJsonObject rec = obj.value(QStringLiteral("recording")).toObject();
    QString recFmt = rec.value(QStringLiteral("format")).toString();
    if (recFmt == QStringLiteral("Gif") || recFmt == QStringLiteral("GIF")) {
        m_recGif->setChecked(true);
    } else {
        m_recMp4->setChecked(true);
    }

    int fps = rec.value(QStringLiteral("fps")).toInt(30);
    QAbstractButton *fpsBtn = m_fpsGroup->button(fps);
    if (fpsBtn) {
        fpsBtn->setChecked(true);
    } else {
        // fallback: select 30
        QAbstractButton *fb = m_fpsGroup->button(30);
        if (fb) fb->setChecked(true);
    }

    QString qual = rec.value(QStringLiteral("quality")).toString();
    int qualIdx = 1; // Medium default
    if (qual == QStringLiteral("Low"))  qualIdx = 0;
    else if (qual == QStringLiteral("High")) qualIdx = 2;
    QAbstractButton *qualBtn = m_recQualGroup->button(qualIdx);
    if (qualBtn) qualBtn->setChecked(true);

    onScreenshotFormatChanged();
    m_statusLabel->clear();
}

void PreferencesWindow::saveConfig()
{
    QJsonObject obj;

    // General
    obj[QStringLiteral("save_directory")]      = m_saveDir->text();
    obj[QStringLiteral("auto_copy_clipboard")] = m_autoCopy->isChecked();
    obj[QStringLiteral("show_notification")]   = m_showNotif->isChecked();
    obj[QStringLiteral("remember_last_region")]= m_rememberRegion->isChecked();

    // Screenshots
    QString fmt = QStringLiteral("Png");
    if (m_fmtJpg->isChecked())       fmt = QStringLiteral("Jpg");
    else if (m_fmtWebp->isChecked()) fmt = QStringLiteral("WebP");
    obj[QStringLiteral("screenshot_format")] = fmt;
    obj[QStringLiteral("jpg_quality")]       = m_quality->value();
    obj[QStringLiteral("filename_pattern")]  = m_filenamePattern->text();

    // Recording sub-object
    QJsonObject rec;

    QString recFmt = m_recGif->isChecked() ? QStringLiteral("Gif") : QStringLiteral("Mp4");
    rec[QStringLiteral("format")] = recFmt;

    int fps = 30;
    QAbstractButton *checkedFps = m_fpsGroup->checkedButton();
    if (checkedFps) {
        fps = m_fpsGroup->id(checkedFps);
    }
    rec[QStringLiteral("fps")] = fps;

    QString qualStr = QStringLiteral("Medium");
    int qualId = m_recQualGroup->checkedId();
    if (qualId == 0)      qualStr = QStringLiteral("Low");
    else if (qualId == 2) qualStr = QStringLiteral("High");
    rec[QStringLiteral("quality")] = qualStr;

    obj[QStringLiteral("recording")] = rec;

    QByteArray jsonBytes = QJsonDocument(obj).toJson(QJsonDocument::Compact);
    int result = snapforge_config_save(jsonBytes.constData());
    if (result == 0) {
        m_statusLabel->setText(QStringLiteral("Saved."));
        QTimer::singleShot(3000, this, [this]() { m_statusLabel->clear(); });
    } else {
        m_statusLabel->setText(QStringLiteral("Error: failed to save config."));
    }
}
