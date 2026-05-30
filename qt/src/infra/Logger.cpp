#include "Logger.h"

#include <QCoreApplication>
#include <QDir>
#include <QLoggingCategory>
#include <QMutexLocker>
#include <QStandardPaths>
#include <QSysInfo>
#include <QThread>
#include <QtGlobal>

#include <csignal>
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
    QString text = msg;
    if (type >= QtWarningMsg && ctx.file) {
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
int g_crashFd = -1;

void writeRaw(const char *s) {
    if (g_crashFd >= 0) { ssize_t n = ::write(g_crashFd, s, strlen(s)); (void)n; }
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

    // Log everything during development: enable all app categories down to debug,
    // mute only Qt's own noisy internals.
    QLoggingCategory::setFilterRules(QStringLiteral(
        "*.debug=true\n"
        "qt.*.debug=false\n"
        "qt.*.info=false\n"));

    qInstallMessageHandler(qtMessageHandler);
    l->installCrashHandlers();
}

Logger::Logger(QObject *parent) : QObject(parent) {
    QDir().mkpath(logDir());
    m_filePath = logDir() + QStringLiteral("/snapforge.log");
    m_file.setFileName(m_filePath);
    m_file.open(QIODevice::Append | QIODevice::Text);
    if (m_file.isOpen()) {
        m_fd = m_file.handle();
        g_crashFd = m_fd;
    }
    rotateIfNeeded();
}

Logger::~Logger() {
    QMutexLocker lock(&m_mutex);
    if (m_file.isOpen()) m_file.close();
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
            if (m_file.isOpen()) { m_fd = m_file.handle(); g_crashFd = m_fd; }
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
    if (m_file.isOpen()) { m_fd = m_file.handle(); g_crashFd = m_fd; }
}
