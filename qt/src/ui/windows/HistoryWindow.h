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

struct HistoryEntry {
    QString path;
    QString timestamp;
    QString thumbnailPath;
};

class HistoryCard : public QFrame {
    Q_OBJECT
public:
    explicit HistoryCard(const HistoryEntry &entry, QWidget *parent = nullptr);
    const QString &filePath() const { return m_path; }
    const QString &timestamp() const { return m_timestamp; }
    QString fileName() const;
    bool isImage() const;
    bool isVideo() const;

signals:
    void deleteRequested(const QString &path);
    void showInFolderRequested(const QString &path);
    void copyRequested(const QString &path);

private:
    QString m_path;
    QString m_timestamp;
};

class HistoryWindow : public QWidget {
    Q_OBJECT
public:
    explicit HistoryWindow(QWidget *parent = nullptr);

public slots:
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

    QList<HistoryEntry> m_allEntries;
    QList<HistoryCard *> m_cards;
    int m_colCount = -1;   // grid columns at last (re)flow; -1 forces a flow
    int m_stretchRow = 0;  // row carrying the top-align stretch, reset on reflow
};

#endif // HISTORYWINDOW_H
