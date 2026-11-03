#ifndef SNAPFORGE_LOGGER_H
#define SNAPFORGE_LOGGER_H

#include <QObject>
#include <QString>
#include <QDateTime>
#include <QMutex>
#include <QFile>
#include <QTextStream>
#include <deque>

struct LogEntry {
    QDateTime time;
    QtMsgType level;
    QString category;
    QString message;
    QString thread;     // thread name or id that emitted the entry

    QString formatted() const;
    static QString levelString(QtMsgType t);
};

class Logger : public QObject {
    Q_OBJECT

public:
    static Logger *instance();

    // Install as the global Qt message handler, enable verbose category rules,
    // and arm crash/signal capture. Safe to call multiple times.
    static void install();

    void log(QtMsgType level, const QString &category, const QString &message);

    // Write a session banner (app/version/Qt/OS) to the head of the log.
    void logBanner();

    // Snapshot of the in-memory ring buffer (most recent last).
    QVector<LogEntry> recent() const;

    // Absolute path to the active log file.
    QString filePath() const;

    // Wipe in-memory buffer, truncate the file and delete the rotated
    // snapforge.log.1.
    void clear();

    // Flush + close the file and stop writing to it (and stop the crash
    // handler from using its fd). Called from main() after the message
    // handler is uninstalled, before this static is destroyed. Idempotent.
    void shutdown();

signals:
    void entryAdded(const LogEntry &entry);
    void cleared();

private:
    explicit Logger(QObject *parent = nullptr);
    ~Logger() override;

    void rotateIfNeeded();
    // (Re)open m_file at m_filePath with `mode` and publish its fd to the
    // crash handler only once the open succeeded. Caller holds m_mutex.
    void reopen(QIODevice::OpenMode mode);
    void closeFile();
    void installCrashHandlers();

    mutable QMutex m_mutex;
    std::deque<LogEntry> m_buffer;     // capped ring buffer
    static constexpr int kBufferMax = 5000;
    static constexpr qint64 kMaxFileBytes = 10 * 1024 * 1024; // 10 MiB
    QString m_filePath;
    QFile m_file;
    int m_fd = -1;                     // raw fd for async-signal-safe crash writes
    qint64 m_fileBytes = 0;            // tracked size (QFile::size() would force a flush)
    bool m_shutdown = false;
};

#endif
