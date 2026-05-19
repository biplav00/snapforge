#include "Logger.h"

#include <QCoreApplication>
#include <QDir>
#include <QMutexLocker>
#include <QStandardPaths>
#include <QtGlobal>

namespace {

QString logDir() {
    // ~/Library/Logs/Snapforge on macOS, AppDataLocation/logs elsewhere.
#ifdef Q_OS_MACOS
    QString home = QDir::homePath();
    return home + QStringLiteral("/Library/Logs/Snapforge");
#else
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation)
        + QStringLiteral("/logs");
#endif
}

void qtMessageHandler(QtMsgType type, const QMessageLogContext &ctx, const QString &msg) {
    Logger::instance()->log(type,
                            QString::fromUtf8(ctx.category ? ctx.category : "default"),
                            msg);
    // Also write to stderr in debug builds for tail -f workflows.
#ifndef QT_NO_DEBUG_OUTPUT
    fprintf(stderr, "%s\n", msg.toUtf8().constData());
    fflush(stderr);
#endif
}

} // namespace

QString LogEntry::levelString(QtMsgType t) {
    switch (t) {
        case QtDebugMsg:    return QStringLiteral("DEBUG");
        case QtInfoMsg:     return QStringLiteral("INFO");
        case QtWarningMsg:  return QStringLiteral("WARN");
        case QtCriticalMsg: return QStringLiteral("ERROR");
        case QtFatalMsg:    return QStringLiteral("FATAL");
    }
    return QStringLiteral("?");
}

QString LogEntry::formatted() const {
    return QStringLiteral("%1 [%2] %3: %4")
        .arg(time.toString(Qt::ISODateWithMs),
             levelString(level),
             category,
             message);
}

Logger *Logger::instance() {
    static Logger inst;
    return &inst;
}

void Logger::install() {
    (void)instance();
    qInstallMessageHandler(qtMessageHandler);
}

Logger::Logger(QObject *parent) : QObject(parent) {
    QDir().mkpath(logDir());
    m_filePath = logDir() + QStringLiteral("/snapforge.log");
    m_file.setFileName(m_filePath);
    m_file.open(QIODevice::Append | QIODevice::Text);
    rotateIfNeeded();
}

Logger::~Logger() {
    QMutexLocker lock(&m_mutex);
    if (m_file.isOpen()) m_file.close();
}

QString Logger::filePath() const {
    return m_filePath;
}

void Logger::log(QtMsgType level, const QString &category, const QString &message) {
    LogEntry entry{ QDateTime::currentDateTime(), level, category, message };

    {
        QMutexLocker lock(&m_mutex);
        m_buffer.push_back(entry);
        while ((int)m_buffer.size() > kBufferMax) m_buffer.pop_front();

        if (m_file.isOpen()) {
            QByteArray line = (entry.formatted() + QStringLiteral("\n")).toUtf8();
            m_file.write(line);
            m_file.flush();
            rotateIfNeeded();
        }
    }

    emit entryAdded(entry);
}

QVector<LogEntry> Logger::recent() const {
    QMutexLocker lock(&m_mutex);
    QVector<LogEntry> out;
    out.reserve((int)m_buffer.size());
    for (const auto &e : m_buffer) out.push_back(e);
    return out;
}

void Logger::clear() {
    {
        QMutexLocker lock(&m_mutex);
        m_buffer.clear();
        if (m_file.isOpen()) {
            m_file.close();
            m_file.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text);
        }
    }
    emit cleared();
}

void Logger::rotateIfNeeded() {
    // Caller holds m_mutex.
    if (!m_file.isOpen()) return;
    if (m_file.size() < kMaxFileBytes) return;

    m_file.close();
    QString rotated = m_filePath + QStringLiteral(".1");
    QFile::remove(rotated);
    QFile::rename(m_filePath, rotated);
    m_file.setFileName(m_filePath);
    m_file.open(QIODevice::Append | QIODevice::Text);
}
