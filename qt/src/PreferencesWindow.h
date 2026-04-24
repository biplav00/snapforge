#ifndef PREFERENCESWINDOW_H
#define PREFERENCESWINDOW_H

#include <QWidget>
#include <QCheckBox>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QRadioButton>
#include <QSlider>
#include <QTableWidget>
#include <QButtonGroup>
#include <QKeyEvent>
#include <QScrollArea>
#include <QComboBox>
#include <QMap>
#include <QString>
#include <functional>

// ---------------------------------------------------------------------------
// HotkeyRow — one row in the hotkeys editor
// ---------------------------------------------------------------------------

struct HotkeyRow {
    QString section;       // "global" | "tools" | "sizes" | "actions"
    QString actionKey;     // key inside that section in JSON
    QString displayName;   // human-readable label
    QString shortcut;      // current shortcut string
    QWidget     *badgeWidget  = nullptr;  // shows rendered badges
    QPushButton *changeBtn    = nullptr;  // "Change" / "Press a key..."
    bool recording = false;
};

// ---------------------------------------------------------------------------
// PreferencesWindow
// ---------------------------------------------------------------------------

class PreferencesWindow : public QWidget {
    Q_OBJECT

public:
    explicit PreferencesWindow(QWidget *parent = nullptr);

    // Accessors for main.cpp to read current prefs
    bool autoCopyEnabled() const { return m_autoCopy && m_autoCopy->isChecked(); }
    bool showNotifEnabled() const { return m_showNotif && m_showNotif->isChecked(); }
    bool rememberRegionEnabled() const { return m_rememberRegion && m_rememberRegion->isChecked(); }
    QString saveDirectory() const { return m_saveDir ? m_saveDir->text() : QString(); }
    int screenshotFormat() const { return m_screenshotFmtGroup ? m_screenshotFmtGroup->checkedId() : 0; }
    int jpgQuality() const { return m_quality ? m_quality->value() : 90; }
    QString filenamePattern() const { return m_filenamePattern ? m_filenamePattern->text() : QString(); }

signals:
    void configSaved();

protected:
    void showEvent(QShowEvent *event) override;
    void hideEvent(QHideEvent *event) override;
    void closeEvent(QCloseEvent *event) override;
    bool eventFilter(QObject *obj, QEvent *event) override;

private slots:
    void onBrowseDirectory();
    void onSave();
    void onScreenshotFormatChanged();
    void onResetHotkeys();

private:
    void buildUi();
    QWidget *buildGeneralTab();
    QWidget *buildScreenshotsTab();
    QWidget *buildRecordingTab();
    QWidget *buildHotkeysTab();

    void loadConfig();
    void saveConfig();
    void applyTheme();

    // Hotkey helpers
    void startRecording(int rowIndex);
    void stopRecording(int rowIndex, bool accept, const QString &newShortcut = {});
    QWidget *makeBadgeWidget(const QString &shortcut, QWidget *parent);
    void refreshBadge(int rowIndex);
    static QString formatKeyToShortcut(int key, Qt::KeyboardModifiers mods);

    // Applied stylesheet helpers
    static QString cardButtonStyle(bool selected);
    static QString pillButtonStyle(bool selected);

    // General tab
    QLineEdit   *m_saveDir        = nullptr;
    QCheckBox   *m_autoCopy       = nullptr;
    QCheckBox   *m_showNotif      = nullptr;
    QCheckBox   *m_rememberRegion = nullptr;

    // Screenshots tab — card buttons in exclusive group
    QButtonGroup *m_screenshotFmtGroup = nullptr;
    QPushButton  *m_fmtPngBtn   = nullptr;
    QPushButton  *m_fmtJpgBtn   = nullptr;
    QPushButton  *m_fmtWebpBtn  = nullptr;
    QSlider      *m_quality      = nullptr;
    QLabel       *m_qualityLabel = nullptr;
    QLineEdit    *m_filenamePattern = nullptr;

    // Recording tab — card buttons
    QButtonGroup *m_recFmtGroup   = nullptr;
    QPushButton  *m_recMp4Btn     = nullptr;
    QPushButton  *m_recGifBtn     = nullptr;
    QButtonGroup *m_fpsGroup      = nullptr;
    QButtonGroup *m_recQualGroup  = nullptr;

    // Hotkeys tab
    QVector<HotkeyRow> m_hotkeyRows;
    int  m_recordingRowIndex = -1;  // which row is currently capturing

    // Theme
    QComboBox    *m_themeCombo   = nullptr;

    // Bottom bar
    QLabel       *m_statusLabel  = nullptr;
};

#endif // PREFERENCESWINDOW_H
