#ifndef HISTORYWINDOW_H
#define HISTORYWINDOW_H

#include <QWidget>
#include <QScrollArea>
#include <QLineEdit>
#include <QComboBox>
#include <QLabel>
#include <QPushButton>
#include <QGridLayout>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QFrame>
#include <QList>
#include <QHash>
#include <QImage>
#include <QSet>

class QTimer;

struct HistoryEntry {
    QString path;
    QString timestamp;
    QString thumbnailPath;
};

class HistoryCard : public QFrame {
    Q_OBJECT
public:
    explicit HistoryCard(const HistoryEntry &entry, QWidget *parent = nullptr);
    bool isImage() const;
    bool isVideo() const;
    QString thumbnailPath() const { return m_thumbPath; }
    // Install a decoded thumbnail (already scaled to device pixels with its
    // devicePixelRatio set). A null image shows the "no preview" text.
    void setThumbnail(const QImage &img);

signals:
    void deleteRequested(const QString &path);
    void showInFolderRequested(const QString &path);
    void copyRequested(const QString &path);

private:
    QString m_path;
    QString m_timestamp;
    QString m_thumbPath;
    QLabel *m_thumbLabel = nullptr;
};

class HistoryWindow : public QWidget {
    Q_OBJECT
public:
    explicit HistoryWindow(QWidget *parent = nullptr);

public slots:
    // Coalesced: several calls in one event-loop turn (main.cpp's explicit
    // refresh + our own showEvent) trigger a single reload.
    void refreshHistory();

protected:
    void showEvent(QShowEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;

private slots:
    void onSearchChanged(const QString &text);
    void onFilterChanged(int index);
    void onSortChanged(int index);
    void onClearAll();
    void onDeleteEntry(const QString &path);
    void onShowInFolder(const QString &path);
    void onCopyEntry(const QString &path);

private:
    void setupUi();
    void doRefreshHistory();
    // Fill the card's thumbnail from the cache, or decode it off the GUI
    // thread and fill every card showing that file when it lands.
    void requestThumbnail(HistoryCard *card);
    void flashStatus(const QString &text);
    void loadEntries(const QList<HistoryEntry> &entries);
    // Re-place the EXISTING cards into the grid at the current viewport width.
    // No-op if the column count is unchanged. Resize uses this; only an entry
    // set / filter change goes through loadEntries (card rebuild + thumbnail
    // reload from disk).
    void reflowCards();
    void applyFilters();
    void updateFooter();
    QList<HistoryEntry> parseJson(const QString &json) const;
    QList<HistoryEntry> sortedEntries(QList<HistoryEntry> entries) const;

    QLineEdit   *m_searchEdit  = nullptr;
    QComboBox   *m_filterCombo = nullptr;
    QComboBox   *m_sortCombo   = nullptr;
    QScrollArea *m_scrollArea  = nullptr;
    QWidget     *m_gridWidget  = nullptr;
    QGridLayout *m_gridLayout  = nullptr;
    QLabel      *m_countLabel  = nullptr;
    QPushButton *m_clearAllBtn = nullptr;

    QTimer *m_refreshTimer = nullptr;
    QTimer *m_searchDebounce = nullptr;
    // Decoded thumbnails keyed by "path@dpr", so a search keystroke rebuilding
    // the cards never re-reads the files. Pending decodes are tracked to avoid
    // launching the same one twice.
    QHash<QString, QImage> m_thumbCache;
    QSet<QString> m_thumbPending;

    QList<HistoryEntry> m_allEntries;
    QList<HistoryCard *> m_cards;
    int m_colCount = -1;   // grid columns at last (re)flow; -1 forces a flow
    int m_stretchRow = 0;  // row carrying the top-align stretch, reset on reflow
};

#endif // HISTORYWINDOW_H
