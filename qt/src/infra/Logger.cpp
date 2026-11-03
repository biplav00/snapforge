#include "Logger.h"

#include <QCoreApplication>
#include <QDir>
#include <QLoggingCategory>
#include <QMutexLocker>
#include <QStandardPaths>
#include <QSysInfo>
#include <QThread>
#include <QtGlobal>

#include <atomic>
#include <csignal>
#include <cstring>
#include <cstdlib>
#include <exception>

#include <unistd.h>   // ::write

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

QString currentThreadLabel() {
    QThread *t = QThread::currentThread();
    if (!t) return QStringLiteral("?");
    const QString name = t->objectName();
    if (!name.isEmpty()) return name;
    if (qApp && t == qApp->thread()) return QStringLiteral("main");
    return QStringLiteral("0x%1").arg(reinterpret_cast<quintptr>(t), 0, 16);
}

void qtMessageHandler(QtMsgType type, const QMessageLogContext &ctx, const QString &msg) {
    const char *cat = ctx.category ? ctx.category : "default";

    // Include source location for warnings and above — invaluable while developing.
    // Note: QtMsgType is not severity-ordered (QtInfoMsg=4 > QtWarningMsg=1),
    // so check the warning-or-worse levels explicitly.
    QString text = msg;
    const bool warnOrWorse =
        type == QtWarningMsg || type == QtCriticalMsg || type == QtFatalMsg;
    if (warnOrWorse && ctx.file) {
        text += QStringLiteral("  (%1:%2)")
                    .arg(QString::fromUtf8(ctx.file))
                    .arg(ctx.line);
    }

    Logger::instance()->log(type, QString::fromUtf8(cat), text);

    // Always mirror to stderr so `tail -f` / console workflows see everything
    // during development, in both debug and release builds.
    fprintf(stderr, "%s\n", text.toUtf8().constData());
    fflush(stderr);

    if (type == QtFatalMsg) {
        // qFatal aborts after the handler returns; flush already happened.
        abort();
    }
}

// --- async-signal-safe crash capture -------------------------------------
// Read from signal handlers, so it is a lock-free atomic. It is set to -1
// BEFORE the fd is closed and only published after a successful (re)open, so
// a crash mid-rotation never writes into a closed (or recycled) descriptor.
std::atomic<int> g_crashFd{-1};

void writeRaw(const char *s) {
    const int fd = g_crashFd.load(std::memory_order_relaxed);
    if (fd >= 0) { ssize_t n = ::write(fd, s, strlen(s)); (void)n; }
    ssize_t n2 = ::write(STDERR_FILENO, s, strlen(s)); (void)n2;
}

void crashSignalHandler(int sig) {
    const char *name = "UNKNOWN";
    switch (sig) {
        case SIGSEGV: name = "SIGSEGV"; break;
        case SIGABRT: name = "SIGABRT"; break;
        case SIGBUS:  name = "SIGBUS";  break;
        case SIGILL:  name = "SIGILL";  break;
        case SIGFPE:  name = "SIGFPE";  break;
        default: break;
    }
    writeRaw("\n*** Snapforge crashed: signal ");
    writeRaw(name);
    writeRaw(" ***\n");

    // Restore default handler and re-raise so the OS still produces a crash report.
    signal(sig, SIG_DFL);
    raise(sig);
}

void terminateHandler() {
    writeRaw("\n*** Snapforge terminated: unhandled C++ exception ***\n");
    if (auto ex = std::current_exception()) {
        try { std::rethrow_exception(ex); }
        catch (const std::exception &e) { writeRaw(e.what()); writeRaw("\n"); }
        catch (...) { writeRaw("(non-std exception)\n"); }
    }
    abort();
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
    return QStringLiteral("%1 [%2] %3/%4: %5")
        .arg(time.toString(Qt::ISODateWithMs),
             levelString(level),
             thread,
             category,
             message);
}

Logger *Logger::instance() {
    static Logger inst;
    return &inst;
}

void Logger::install() {
    Logger *l = instance();

    // Debug builds log everything down to debug (muting only Qt's own noisy
    // internals). Release builds log Info and above, so the shipped app
    // doesn't fill the file with per-event debug chatter; set
    // SNAPFORGE_DEBUG_LOG=1 to get debug output from a release build.
#ifdef QT_DEBUG
    const bool verbose = true;
#else
    const bool verbose = qEnvironmentVariableIntValue("SNAPFORGE_DEBUG_LOG") == 1;
#endif
    QLoggingCategory::setFilterRules(verbose
        ? QStringLiteral("*.debug=true\n"
                         "qt.*.debug=false\n"
                         "qt.*.info=false\n")
        : QStringLiteral("*.debug=false\n"
                         "qt.*.info=false\n"));

    qInstallMessageHandler(qtMessageHandler);
    l->installCrashHandlers();
}

Logger::Logger(QObject *parent) : QObject(parent) {
    QDir().mkpath(logDir());
    m_filePath = logDir() + QStringLiteral("/snapforge.log");
    QMutexLocker lock(&m_mutex);
    reopen(QIODevice::Append | QIODevice::Text);
    rotateIfNeeded();
}

Logger::~Logger() {
    shutdown();
}

void Logger::shutdown() {
    QMutexLocker lock(&m_mutex);
    m_shutdown = true;
    closeFile();
}

void Logger::closeFile() {
    // Caller holds m_mutex. Unpublish the fd first (see g_crashFd).
    g_crashFd.store(-1);
    m_fd = -1;
    if (m_file.isOpen()) {
        m_file.flush();
        m_file.close();
    }
}

void Logger::reopen(QIODevice::OpenMode mode) {
    // Caller holds m_mutex.
    closeFile();
    m_file.setFileName(m_filePath);
    if (m_file.open(mode)) {
        m_fileBytes = m_file.size();
        m_fd = m_file.handle();
        g_crashFd.store(m_fd);
    } else {
        m_fileBytes = 0;
    }
}

void Logger::installCrashHandlers() {
    std::set_terminate(terminateHandler);
    for (int sig : {SIGSEGV, SIGABRT, SIGBUS, SIGILL, SIGFPE}) {
        signal(sig, crashSignalHandler);
    }
}

void Logger::logBanner() {
    const QString app = QCoreApplication::applicationName().isEmpty()
                            ? QStringLiteral("Snapforge")
                            : QCoreApplication::applicationName();
    const QString ver = QCoreApplication::applicationVersion();
    log(QtInfoMsg, QStringLiteral("session"),
        QStringLiteral("======== %1 %2 started ========").arg(app, ver));
    log(QtInfoMsg, QStringLiteral("session"),
        QStringLiteral("Qt %1 | %2 %3 | %4")
            .arg(QStringLiteral(QT_VERSION_STR),
                 QSysInfo::prettyProductName(),
                 QSysInfo::currentCpuArchitecture(),
                 QSysInfo::kernelVersion()));
    log(QtInfoMsg, QStringLiteral("session"),
        QStringLiteral("Log file: %1").arg(m_filePath));
}

QString Logger::filePath() const {
    return m_filePath;
}

void Logger::log(QtMsgType level, const QString &category, const QString &message) {
    LogEntry entry{ QDateTime::currentDateTime(), level, category, message,
                    currentThreadLabel() };

    {
        QMutexLocker lock(&m_mutex);
        m_buffer.push_back(entry);
        while ((int)m_buffer.size() > kBufferMax) m_buffer.pop_front();

        if (!m_shutdown && m_file.isOpen()) {
            QByteArray line = (entry.formatted() + QStringLiteral("\n")).toUtf8();
            m_file.write(line);
            m_fileBytes += line.size();
            // Debug lines stay in QFile's buffer (cheap; flushed by the next
            // info+ line or at shutdown). Info and above hit the disk
            // immediately so the lines leading up to a crash are on disk —
            // the crash handler writes the raw fd and can't flush QFile.
            if (level != QtDebugMsg)
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
        QFile::remove(m_filePath + QStringLiteral(".1"));
        if (!m_shutdown && m_file.isOpen())
            reopen(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text);
    }
    emit cleared();
}

void Logger::rotateIfNeeded() {
    // Caller holds m_mutex.
    if (!m_file.isOpen()) return;
    if (m_fileBytes < kMaxFileBytes) return;

    closeFile();
    QString rotated = m_filePath + QStringLiteral(".1");
    QFile::remove(rotated);
    QFile::rename(m_filePath, rotated);
    reopen(QIODevice::Append | QIODevice::Text);
}
