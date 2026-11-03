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
#include <QPlainTextEdit>
#include <QJsonObject>
#include <QRect>
#include <functional>

class QTabWidget;
class QTimer;

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

    // Accessors for main.cpp to read current prefs. These return the SAVED
    // configuration (loaded at construction, refreshed on every load/save),
    // never the live widget state — so prefs apply from app start without the
    // window ever being opened, and unsaved edits don't leak into behaviour.
    bool autoCopyEnabled() const { return m_saved.autoCopy; }
    bool showNotifEnabled() const { return m_saved.showNotif; }
    bool rememberRegionEnabled() const { return m_saved.rememberRegion; }
    QString saveDirectory() const { return m_saved.saveDir; }
    int screenshotFormat() const { return m_saved.screenshotFormat; }
    int jpgQuality() const { return m_saved.jpgQuality; }
    QString filenamePattern() const { return m_saved.filenamePattern; }
    bool showClicksEnabled() const { return m_saved.showClicks; }

    // Convert Qt key + modifiers (+ the hardware virtual keycode, 0 if
    // unknown) to a chord string like "Cmd+Shift+S". Public for tests.
    // Qt swaps Command/Control on macOS: ControlModifier (⌘) -> "Cmd",
    // MetaModifier (physical ⌃) -> "Ctrl". Shifted digits/punctuation are
    // recorded as their unshifted key ("Shift+1", not "Shift+!"), and the plus
    // key is spelled "Plus". Returns an empty string for keys that can't be
    // represented.
    static QString formatKeyToShortcut(int key, Qt::KeyboardModifiers mods,
                                       quint32 nativeVirtualKey = 0);

    // Remembered region (window-local coords), persisted across restarts. Empty
    // if none saved yet.
    QRect lastRegion() const { return m_lastRegion; }
    // Persist a new remembered region immediately (merge-saved so it doesn't
    // clobber unsaved edits in the open prefs window).
    void setLastRegion(const QRect &region);

signals:
    void configSaved();
    // True while the hotkey recorder is capturing a chord (keyboard grabbed),
    // false once it stops — accepted, aborted, or the window hides. main.cpp
    // unregisters the Carbon global hotkeys while active so pressing an
    // existing global chord records it instead of firing the action.
    void hotkeyCaptureActive(bool active);

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
    // Theme change: every tab bakes the current theme into its children's
    // stylesheets at construction, so a live palette flip tears the tab
    // contents down and re-runs buildUi(), preserving the visible tab and any
    // unsaved edits. See applyTheme().
    void rebuildThemedUi();
    QWidget *buildGeneralTab();
    QWidget *buildScreenshotsTab();
    QWidget *buildRecordingTab();
    QWidget *buildHotkeysTab();
    QWidget *buildPermissionsTab();
    QWidget *buildLogsTab();
    void refreshPermissionStatus();
    void appendLogLine(const QString &line, QtMsgType level);
    void reloadLogBuffer();

    void loadConfig();
    bool saveConfig();
    // Copy the (just loaded or saved) widget values into m_saved.
    void snapshotSavedPrefs();

public:
    // Fix #20: re-run the current theme preference. Called from a
    // QApplication event filter when the system palette changes so "Auto"
    // follows system dark/light mode, while explicit Light/Dark ignore it.
    void applyTheme();

private:

    // Hotkey helpers
    void startRecording(int rowIndex);
    void stopRecording(int rowIndex, bool accept, const QString &newShortcut = {});
    void refreshBadge(int rowIndex);
    // Validate the hotkey rows + filename pattern before saving. Returns an
    // empty string if OK, else a user-facing error message.
    QString validateBeforeSave() const;
    void showStatus(const QString &text, bool error, int clearAfterMs = 3000);

    // General tab
    QLineEdit   *m_saveDir        = nullptr;
    QCheckBox   *m_autoCopy       = nullptr;
    QCheckBox   *m_showNotif      = nullptr;
    QCheckBox   *m_rememberRegion = nullptr;
    // Persisted remembered region (not a widget — loaded/saved with the config).
    QRect        m_lastRegion;

    // Snapshot of the last successfully loaded/saved configuration. The
    // public getters read from here (see above).
    struct SavedPrefs {
        QString saveDir;
        bool    autoCopy = false;
        bool    showNotif = false;
        bool    rememberRegion = false;
        int     screenshotFormat = 0; // 0 = PNG, 1 = JPG, 2 = WebP
        int     jpgQuality = 90;
        QString filenamePattern;
        bool    showClicks = false;
    };
    SavedPrefs   m_saved;
    // Full JSON of the last good load/save. Seed for merge-saves when the
    // on-disk config can't be read, so a transient load failure never writes
    // back a partial object that wipes the user's settings.
    QJsonObject  m_savedJson;

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
    QCheckBox    *m_showClicks    = nullptr;

    // Hotkeys tab
    QVector<HotkeyRow> m_hotkeyRows;
    int  m_recordingRowIndex = -1;  // which row is currently capturing

    // Theme
    QComboBox    *m_themeCombo   = nullptr;
    QTabWidget   *m_tabs         = nullptr;

    // Permissions tab
    QLabel       *m_screenRecStatusBadge = nullptr;
    QLabel       *m_screenRecHelpText    = nullptr;
    QPushButton  *m_screenRecRequestBtn  = nullptr;
    QPushButton  *m_screenRecOpenSettingsBtn = nullptr;
    QPushButton  *m_screenRecRefreshBtn  = nullptr;

    // Bottom bar
    QLabel       *m_statusLabel  = nullptr;

    // Logs tab
    QPlainTextEdit *m_logView      = nullptr;
    QComboBox      *m_logLevelFilter = nullptr;
    QCheckBox      *m_logAutoScroll  = nullptr;
    QLabel         *m_logPathLabel   = nullptr;
    QLineEdit      *m_logSearch      = nullptr;
    QTimer         *m_logSearchDebounce = nullptr;
};

#endif // PREFERENCESWINDOW_H
