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

class PreferencesWindow : public QWidget {
    Q_OBJECT
public:
    explicit PreferencesWindow(QWidget *parent = nullptr);

protected:
    void showEvent(QShowEvent *event) override;

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

    // General tab
    QLineEdit   *m_saveDir       = nullptr;
    QCheckBox   *m_autoCopy      = nullptr;
    QCheckBox   *m_showNotif     = nullptr;
    QCheckBox   *m_rememberRegion = nullptr;

    // Screenshots tab
    QRadioButton *m_fmtPng       = nullptr;
    QRadioButton *m_fmtJpg       = nullptr;
    QRadioButton *m_fmtWebp      = nullptr;
    QSlider      *m_quality      = nullptr;
    QLabel       *m_qualityLabel = nullptr;
    QLineEdit    *m_filenamePattern = nullptr;

    // Recording tab
    QRadioButton *m_recMp4       = nullptr;
    QRadioButton *m_recGif       = nullptr;
    QButtonGroup *m_fpsGroup     = nullptr;
    QButtonGroup *m_recQualGroup = nullptr;

    // Hotkeys tab
    QTableWidget *m_hotkeysTable = nullptr;

    // Bottom bar
    QLabel       *m_statusLabel  = nullptr;
};

#endif // PREFERENCESWINDOW_H
