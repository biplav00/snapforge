#include "HistoryWindow.h"
#include "snapforge_ffi.h"

#include <QApplication>
#include <QClipboard>
#include <QDesktopServices>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QPixmap>
#include <QPushButton>
#include <QScrollArea>
#include <QShowEvent>
#include <QUrl>
#include <QVBoxLayout>
#include <algorithm>

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static bool extensionIsImage(const QString &ext) {
    static const QStringList imgs = {"png", "jpg", "jpeg", "webp"};
    return imgs.contains(ext.toLower());
}

static bool extensionIsVideo(const QString &ext) {
    static const QStringList vids = {"mp4", "mov", "m4v", "webm"};
    return vids.contains(ext.toLower());
}

// ---------------------------------------------------------------------------
// HistoryCard
// ---------------------------------------------------------------------------

HistoryCard::HistoryCard(const HistoryEntry &entry, QWidget *parent)
    : QFrame(parent)
    , m_path(entry.path)
    , m_timestamp(entry.timestamp)
{
    setFrameShape(QFrame::StyledPanel);
    setFixedWidth(200);

    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(8, 8, 8, 8);
    root->setSpacing(6);

    // --- Thumbnail ---
    auto *thumbLabel = new QLabel(this);
    thumbLabel->setFixedHeight(120);
    thumbLabel->setAlignment(Qt::AlignCenter);
    thumbLabel->setScaledContents(false);

    if (!entry.thumbnailPath.isEmpty()) {
        QPixmap px(entry.thumbnailPath);
        if (!px.isNull()) {
            thumbLabel->setPixmap(px.scaled(184, 120, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        } else {
            thumbLabel->setText(isVideo() ? "[video]" : "[no preview]");
        }
    } else {
        thumbLabel->setText(isVideo() ? "[video]" : "[no preview]");
    }
    root->addWidget(thumbLabel);

    // --- Filename ---
    QString name = QFileInfo(m_path).fileName();
    auto *nameLabel = new QLabel(name, this);
    nameLabel->setWordWrap(false);
    nameLabel->setToolTip(m_path);
    {
        QFont f = nameLabel->font();
        f.setPointSize(10);
        nameLabel->setFont(f);
    }
    // Elide long names visually via fixed width
    nameLabel->setMaximumWidth(184);
    root->addWidget(nameLabel);

    // --- Timestamp ---
    if (!m_timestamp.isEmpty()) {
        auto *tsLabel = new QLabel(m_timestamp, this);
        {
            QFont f = tsLabel->font();
            f.setPointSize(9);
            tsLabel->setFont(f);
        }
        tsLabel->setStyleSheet("color: gray;");
        root->addWidget(tsLabel);
    }

    // --- Action buttons ---
    auto *btnRow = new QHBoxLayout();
    btnRow->setSpacing(4);

    auto *showBtn = new QPushButton("Show", this);
    showBtn->setToolTip("Show in Folder");
    showBtn->setFixedHeight(24);
    connect(showBtn, &QPushButton::clicked, this, [this]() {
        emit showInFolderRequested(m_path);
    });
    btnRow->addWidget(showBtn);

    if (isImage()) {
        auto *copyBtn = new QPushButton("Copy", this);
        copyBtn->setFixedHeight(24);
        connect(copyBtn, &QPushButton::clicked, this, [this]() {
            emit copyRequested(m_path);
        });
        btnRow->addWidget(copyBtn);
    }

    auto *delBtn = new QPushButton("Del", this);
    delBtn->setToolTip("Delete");
    delBtn->setFixedHeight(24);
    delBtn->setStyleSheet("color: #c0392b;");
    connect(delBtn, &QPushButton::clicked, this, [this]() {
        emit deleteRequested(m_path);
    });
    btnRow->addWidget(delBtn);

    root->addLayout(btnRow);
}

QString HistoryCard::fileName() const {
    return QFileInfo(m_path).fileName();
}

bool HistoryCard::isImage() const {
    return extensionIsImage(QFileInfo(m_path).suffix());
}

bool HistoryCard::isVideo() const {
    return extensionIsVideo(QFileInfo(m_path).suffix());
}

// ---------------------------------------------------------------------------
// HistoryWindow
// ---------------------------------------------------------------------------

HistoryWindow::HistoryWindow(QWidget *parent)
    : QWidget(parent)
{
    setWindowTitle("Snapforge History");
    resize(960, 640);
    setMinimumSize(600, 400);

    setupUi();
}

void HistoryWindow::setupUi() {
    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(12, 12, 12, 12);
    root->setSpacing(8);

    // --- Toolbar ---
    auto *toolbar = new QHBoxLayout();
    toolbar->setSpacing(8);

    m_searchEdit = new QLineEdit(this);
    m_searchEdit->setPlaceholderText("Search...");
    m_searchEdit->setMinimumWidth(180);
    connect(m_searchEdit, &QLineEdit::textChanged, this, &HistoryWindow::onSearchChanged);
    toolbar->addWidget(m_searchEdit);

    m_filterCombo = new QComboBox(this);
    m_filterCombo->addItems({"All", "Images", "Videos"});
    connect(m_filterCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &HistoryWindow::onFilterChanged);
    toolbar->addWidget(m_filterCombo);

    m_sortCombo = new QComboBox(this);
    m_sortCombo->addItems({"Newest", "Oldest", "Name"});
    connect(m_sortCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &HistoryWindow::onSortChanged);
    toolbar->addWidget(m_sortCombo);

    toolbar->addStretch();
    root->addLayout(toolbar);

    // --- Separator ---
    auto *sep = new QFrame(this);
    sep->setFrameShape(QFrame::HLine);
    sep->setFrameShadow(QFrame::Sunken);
    root->addWidget(sep);

    // --- Scroll area with grid ---
    m_scrollArea = new QScrollArea(this);
    m_scrollArea->setWidgetResizable(true);
    m_scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    m_scrollArea->setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);

    m_gridWidget = new QWidget();
    m_gridLayout = new QGridLayout(m_gridWidget);
    m_gridLayout->setSpacing(12);
    m_gridLayout->setContentsMargins(8, 8, 8, 8);
    m_gridLayout->setAlignment(Qt::AlignTop | Qt::AlignLeft);

    m_scrollArea->setWidget(m_gridWidget);
    root->addWidget(m_scrollArea, 1);

    // --- Footer ---
    auto *footer = new QHBoxLayout();
    footer->setSpacing(8);

    m_countLabel = new QLabel("0 items", this);
    footer->addWidget(m_countLabel);
    footer->addStretch();

    m_clearAllBtn = new QPushButton("Clear All", this);
    connect(m_clearAllBtn, &QPushButton::clicked, this, &HistoryWindow::onClearAll);
    footer->addWidget(m_clearAllBtn);

    root->addLayout(footer);
}

void HistoryWindow::showEvent(QShowEvent *event) {
    QWidget::showEvent(event);
    refreshHistory();
}

void HistoryWindow::refreshHistory() {
    char *raw = snapforge_history_list();
    if (!raw) {
        loadEntries({});
        return;
    }
    QString json = QString::fromUtf8(raw);
    snapforge_free_string(raw);

    QList<HistoryEntry> entries = parseJson(json);
    m_allEntries = entries;
    applyFilters();
}

QList<HistoryEntry> HistoryWindow::parseJson(const QString &json) const {
    QList<HistoryEntry> result;
    QJsonDocument doc = QJsonDocument::fromJson(json.toUtf8());
    if (!doc.isArray()) return result;

    const QJsonArray arr = doc.array();
    for (const QJsonValue &val : arr) {
        if (!val.isObject()) continue;
        QJsonObject obj = val.toObject();
        HistoryEntry e;
        e.path          = obj.value("path").toString();
        e.timestamp     = obj.value("timestamp").toString();
        e.thumbnailPath = obj.value("thumbnail_path").toString();
        if (!e.path.isEmpty()) {
            result.append(e);
        }
    }
    return result;
}

QList<HistoryEntry> HistoryWindow::sortedEntries(QList<HistoryEntry> entries) const {
    int sortIdx = m_sortCombo ? m_sortCombo->currentIndex() : 0;

    if (sortIdx == 0) {
        // Newest first — sort by timestamp descending
        std::stable_sort(entries.begin(), entries.end(), [](const HistoryEntry &a, const HistoryEntry &b) {
            return a.timestamp > b.timestamp;
        });
    } else if (sortIdx == 1) {
        // Oldest first
        std::stable_sort(entries.begin(), entries.end(), [](const HistoryEntry &a, const HistoryEntry &b) {
            return a.timestamp < b.timestamp;
        });
    } else {
        // By name
        std::stable_sort(entries.begin(), entries.end(), [](const HistoryEntry &a, const HistoryEntry &b) {
            return QFileInfo(a.path).fileName().toLower() < QFileInfo(b.path).fileName().toLower();
        });
    }
    return entries;
}

void HistoryWindow::applyFilters() {
    const QString search    = m_searchEdit   ? m_searchEdit->text().toLower()   : QString();
    const int     filterIdx = m_filterCombo  ? m_filterCombo->currentIndex()    : 0;

    QList<HistoryEntry> filtered;
    for (const HistoryEntry &e : m_allEntries) {
        // Search filter
        if (!search.isEmpty()) {
            QString fname = QFileInfo(e.path).fileName().toLower();
            if (!fname.contains(search)) continue;
        }

        // Type filter
        QString ext = QFileInfo(e.path).suffix();
        if (filterIdx == 1 && !extensionIsImage(ext)) continue;
        if (filterIdx == 2 && !extensionIsVideo(ext)) continue;

        filtered.append(e);
    }

    loadEntries(sortedEntries(filtered));
}

void HistoryWindow::loadEntries(const QList<HistoryEntry> &entries) {
    // Remove old cards
    for (HistoryCard *c : m_cards) {
        m_gridLayout->removeWidget(c);
        c->deleteLater();
    }
    m_cards.clear();

    // Calculate columns based on window width
    // Each card is 200px wide + 12px spacing; recalculate on next paint.
    // For simplicity, use a fixed 4-column max grid; the scroll area handles overflow.
    const int colCount = qMax(1, (m_scrollArea->viewport()->width() - 8) / (200 + 12));

    int row = 0, col = 0;
    for (const HistoryEntry &e : entries) {
        auto *card = new HistoryCard(e, m_gridWidget);
        connect(card, &HistoryCard::deleteRequested,      this, &HistoryWindow::onDeleteEntry);
        connect(card, &HistoryCard::showInFolderRequested, this, &HistoryWindow::onShowInFolder);
        connect(card, &HistoryCard::copyRequested,        this, &HistoryWindow::onCopyEntry);

        m_gridLayout->addWidget(card, row, col, Qt::AlignTop | Qt::AlignLeft);
        m_cards.append(card);

        col++;
        if (col >= colCount) {
            col = 0;
            row++;
        }
    }

    // Add a stretch spacer at the end to push cards to top-left
    m_gridLayout->setRowStretch(row + 1, 1);

    updateFooter();
}

void HistoryWindow::updateFooter() {
    if (m_countLabel) {
        int n = m_cards.size();
        m_countLabel->setText(QString("%1 item%2").arg(n).arg(n == 1 ? "" : "s"));
    }
}

// --- Slots ---

void HistoryWindow::onSearchChanged(const QString &) {
    applyFilters();
}

void HistoryWindow::onFilterChanged(int) {
    applyFilters();
}

void HistoryWindow::onSortChanged(int) {
    applyFilters();
}

void HistoryWindow::onClearAll() {
    if (m_allEntries.isEmpty()) return;

    QMessageBox::StandardButton btn = QMessageBox::question(
        this, "Clear History",
        "Remove all history entries? Files will not be deleted from disk.",
        QMessageBox::Yes | QMessageBox::Cancel, QMessageBox::Cancel);

    if (btn != QMessageBox::Yes) return;

    snapforge_history_clear();
    m_allEntries.clear();
    loadEntries({});
}

void HistoryWindow::onDeleteEntry(const QString &path) {
    snapforge_history_delete(path.toUtf8().constData());

    // Remove from master list
    m_allEntries.erase(
        std::remove_if(m_allEntries.begin(), m_allEntries.end(),
                       [&path](const HistoryEntry &e) { return e.path == path; }),
        m_allEntries.end());

    applyFilters();
}

void HistoryWindow::onShowInFolder(const QString &path) {
    QString dir = QFileInfo(path).absolutePath();
    QDesktopServices::openUrl(QUrl::fromLocalFile(dir));
}

void HistoryWindow::onCopyEntry(const QString &path) {
    QImage img(path);
    if (img.isNull()) return;

    // Convert to RGBA8888 for FFI
    QImage rgba = img.convertToFormat(QImage::Format_RGBA8888);
    snapforge_copy_to_clipboard(rgba.constBits(), static_cast<uint32_t>(rgba.width()),
                                static_cast<uint32_t>(rgba.height()));
}
