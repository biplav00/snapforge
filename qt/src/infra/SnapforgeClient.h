#ifndef SNAPFORGECLIENT_H
#define SNAPFORGECLIENT_H

// SnapforgeClient — the Qt-side adapter that owns the FFI seam.
//
// This is the *only* header any widget / window / controller includes to reach
// the Rust core. Its two implementations satisfy the same interface and are
// swapped at LINK time:
//   * SnapforgeClient.cpp     — real, the only TU that #includes snapforge_ffi.h
//   * SnapforgeClientFake.cpp — in-memory, linked into test targets instead
//
// The interface absorbs the three things every old call site used to do by
// hand: build the request JSON, read snapforge_app_last_error(), and free the
// returned C string / pixel buffer. Callers see typed Qt values only.
//
// Error model: a value call returns std::optional (empty == failed) and a
// success/fail call returns bool; on failure read sf::lastError() /
// sf::lastErrorCode() immediately (they reflect the most recent use-case call,
// mirroring the FFI's single-error contract). Capture/config/history
// primitives do not populate the use-case error channel.

#include <QByteArray>
#include <QImage>
#include <QString>
#include <cstddef>
#include <cstdint>
#include <optional>

namespace sf {

// --- Opaque handles ---------------------------------------------------------
// Distinct wrappers (not bare void*) so a click handle can't be passed to a
// recording call. `valid()` is false for a failed start.
struct RecHandle {
    void *p = nullptr;
    bool valid() const { return p != nullptr; }
};
struct ClickHandle {
    void *p = nullptr;
    bool valid() const { return p != nullptr; }
};

// Click callback signature — layout-compatible with the FFI's
// SnapforgeClickCallback. Fires on a Rust-owned thread; the consumer
// re-dispatches to the Qt thread (see ClickTap).
using ClickCallback = void (*)(double x, double y, int rightClick, void *userData);
// Log callback signature — layout-compatible with SnapforgeLogCallback.
using LogCallback = void (*)(int level, const char *msg);
// The `level` int a LogCallback receives — mirrors the FFI's SnapforgeLogLevel
// so callers can switch on it without including snapforge_ffi.h.
enum LogLevel { LogTrace = 0, LogDebug = 1, LogInfo = 2, LogWarn = 3, LogError = 4 };

// --- Typed requests ---------------------------------------------------------
struct Region {
    int32_t x = 0;
    int32_t y = 0;
    uint32_t width = 0;
    uint32_t height = 0;
};

struct ScreenshotReq {
    uint32_t display = 0;
    std::optional<Region> region;          // nullopt == fullscreen
    QString outputPath;
    QString format = QStringLiteral("png"); // "png"/"jpg"/"webp"
    int quality = 90;                        // 1..=100
    bool copyToClipboard = false;
    bool addToHistory = false;
};

struct SaveReq {
    QString outputPath;                      // empty == clipboard-only
    QString format = QStringLiteral("png");
    int quality = 90;
    bool copyToClipboard = false;
    bool addToHistory = false;
};

struct RecordReq {
    uint32_t display = 0;
    std::optional<Region> region;
    QString outputPath;
    QString format = QStringLiteral("mp4");  // "mp4"/"gif"
    uint32_t fps = 30;
    QString quality = QStringLiteral("medium"); // "low"/"medium"/"high"
    std::optional<QString> ffmpegPath;
    bool addToHistoryOnStop = false;
    bool showClicks = false;  // composite click ripples into the recording
};

// --- Error channel ----------------------------------------------------------
QString lastError();      // message of the most recent failed use-case call
int lastErrorCode();      // SnapforgeErrorCode category (0 == none)

// --- Capture / permissions / display / paths --------------------------------
std::optional<QImage> captureFullscreen(uint32_t display);
std::optional<QImage> captureRegion(uint32_t display, const Region &region);
bool hasPermission();
bool requestPermission();
int displayAtPoint(int32_t x, int32_t y);   // display index, or -1
double displayScaleFactor();
QString defaultSavePath();

// --- History ----------------------------------------------------------------
QByteArray historyListJson();                // empty QByteArray on error
bool historyDelete(const QString &path);
bool historyClear();
bool isIncompleteMp4(const QString &path);

// --- Config (JSON passthrough; the window parses it for the UI) -------------
QByteArray configLoadJson();                 // empty QByteArray on error
bool configSave(const QByteArray &json);

// --- Use cases --------------------------------------------------------------
// takeScreenshot/savePrerendered return the saved path. For savePrerendered a
// present-but-empty string means a clipboard-only success (no file written);
// nullopt means failure.
std::optional<QString> takeScreenshot(const ScreenshotReq &req);
std::optional<QString> savePrerendered(const uint8_t *rgba, std::size_t rgbaLen,
                                       uint32_t width, uint32_t height,
                                       const SaveReq &req);

RecHandle recordStart(const RecordReq &req);
bool recordStop(RecHandle handle);
bool recordPause(RecHandle handle);
bool recordResume(RecHandle handle);
void recordFreeHandle(RecHandle handle);

// --- Clicks (callback + opaque handle passthrough) --------------------------
ClickHandle clicksStart(ClickCallback callback, void *userData);
bool clicksStop(ClickHandle handle);
void clicksFreeHandle(ClickHandle handle);

// --- Logging bridge ---------------------------------------------------------
void setLogCallback(LogCallback callback);

} // namespace sf

#endif // SNAPFORGECLIENT_H
