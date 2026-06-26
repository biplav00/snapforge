// Real SnapforgeClient — translates typed Qt calls into the snapforge_* FFI.
// This is the ONLY translation unit that includes snapforge_ffi.h; the fake
// (SnapforgeClientFake.cpp) provides the same symbols for test targets.

#include "SnapforgeClient.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>

#include "snapforge_ffi.h"

namespace sf {
namespace {

// Take ownership of a Rust-allocated C string: copy to QString, free, return.
QString takeOwnedString(char *s) {
    if (!s) return QString();
    QString out = QString::fromUtf8(s);
    snapforge_free_string(s);
    return out;
}

void putRegion(QJsonObject &obj, const std::optional<Region> &region) {
    if (!region) return;
    QJsonObject r;
    r["x"] = region->x;
    r["y"] = region->y;
    r["width"] = static_cast<qint64>(region->width);
    r["height"] = static_cast<qint64>(region->height);
    obj["region"] = r;
}

QByteArray toJson(const QJsonObject &obj) {
    return QJsonDocument(obj).toJson(QJsonDocument::Compact);
}

// Parse {"saved_path": "..."|null} -> QString (null/missing -> empty string).
std::optional<QString> parseSavedPath(char *resultJson) {
    if (!resultJson) return std::nullopt; // failure; see lastError()
    const QString json = takeOwnedString(resultJson);
    const QJsonObject obj = QJsonDocument::fromJson(json.toUtf8()).object();
    const QJsonValue v = obj.value("saved_path");
    return v.isString() ? v.toString() : QString();
}

} // namespace

QString lastError() { return takeOwnedString(snapforge_app_last_error()); }
int lastErrorCode() { return snapforge_app_last_error_code(); }

std::optional<QImage> captureFullscreen(uint32_t display) {
    return [&]() -> std::optional<QImage> {
        CapturedImage img = snapforge_capture_fullscreen(display);
        if (!img.data || img.width == 0 || img.height == 0) return std::nullopt;
        // QImage(ptr, ...) does not copy — .copy() detaches into owned memory
        // so we can free the FFI buffer immediately.
        QImage owned = QImage(img.data, static_cast<int>(img.width),
                              static_cast<int>(img.height),
                              QImage::Format_RGBA8888)
                           .copy();
        snapforge_free_buffer(img.data, img.len);
        return owned;
    }();
}

std::optional<QImage> captureRegion(uint32_t display, const Region &region) {
    CapturedImage img = snapforge_capture_region(display, region.x, region.y,
                                                 region.width, region.height);
    if (!img.data || img.width == 0 || img.height == 0) return std::nullopt;
    QImage owned = QImage(img.data, static_cast<int>(img.width),
                          static_cast<int>(img.height),
                          QImage::Format_RGBA8888)
                       .copy();
    snapforge_free_buffer(img.data, img.len);
    return owned;
}

bool hasPermission() { return snapforge_has_permission() == 1; }
bool requestPermission() { return snapforge_request_permission() == 1; }
int displayAtPoint(int32_t x, int32_t y) { return snapforge_display_at_point(x, y); }
double displayScaleFactor() { return snapforge_display_scale_factor(); }
QString defaultSavePath() { return takeOwnedString(snapforge_default_save_path()); }

QByteArray historyListJson() {
    char *s = snapforge_history_list();
    if (!s) return QByteArray();
    QByteArray out = QByteArray(s);
    snapforge_free_string(s);
    return out;
}

bool historyDelete(const QString &path) {
    return snapforge_history_delete(path.toUtf8().constData()) == 0;
}
bool historyClear() { return snapforge_history_clear() == 0; }
bool isIncompleteMp4(const QString &path) {
    return snapforge_is_incomplete_mp4(path.toUtf8().constData()) == 1;
}

QByteArray configLoadJson() {
    char *s = snapforge_config_load();
    if (!s) return QByteArray();
    QByteArray out = QByteArray(s);
    snapforge_free_string(s);
    return out;
}
bool configSave(const QByteArray &json) {
    return snapforge_config_save(json.constData()) == 0;
}

std::optional<QString> takeScreenshot(const ScreenshotReq &req) {
    QJsonObject o;
    o["display"] = static_cast<qint64>(req.display);
    putRegion(o, req.region);
    o["output_path"] = req.outputPath;
    o["format"] = req.format;
    o["quality"] = req.quality;
    o["copy_to_clipboard"] = req.copyToClipboard;
    o["add_to_history"] = req.addToHistory;
    return parseSavedPath(snapforge_screenshot(toJson(o).constData()));
}

std::optional<QString> savePrerendered(const uint8_t *rgba, std::size_t rgbaLen,
                                       uint32_t width, uint32_t height,
                                       const SaveReq &req) {
    QJsonObject o;
    if (!req.outputPath.isEmpty()) o["output_path"] = req.outputPath;
    o["format"] = req.format;
    o["quality"] = req.quality;
    o["copy_to_clipboard"] = req.copyToClipboard;
    o["add_to_history"] = req.addToHistory;
    char *res = snapforge_save_prerendered(rgba, rgbaLen, width, height,
                                           toJson(o).constData());
    return parseSavedPath(res);
}

RecHandle recordStart(const RecordReq &req) {
    QJsonObject o;
    o["display"] = static_cast<qint64>(req.display);
    putRegion(o, req.region);
    o["output_path"] = req.outputPath;
    o["format"] = req.format;
    o["fps"] = static_cast<qint64>(req.fps);
    o["quality"] = req.quality;
    if (req.ffmpegPath) o["ffmpeg_path"] = *req.ffmpegPath;
    o["add_to_history_on_stop"] = req.addToHistoryOnStop;
    return RecHandle{snapforge_record_start(toJson(o).constData())};
}

bool recordStop(RecHandle handle) { return snapforge_record_stop(handle.p) == 0; }
bool recordPause(RecHandle handle) { return snapforge_record_pause(handle.p) == 0; }
bool recordResume(RecHandle handle) { return snapforge_record_resume(handle.p) == 0; }
void recordFreeHandle(RecHandle handle) { snapforge_record_free_handle(handle.p); }

ClickHandle clicksStart(ClickCallback callback, void *userData) {
    // sf::ClickCallback and SnapforgeClickCallback have the same signature.
    return ClickHandle{snapforge_clicks_start(
        reinterpret_cast<SnapforgeClickCallback>(callback), userData)};
}
bool clicksStop(ClickHandle handle) { return snapforge_clicks_stop(handle.p) == 0; }
void clicksFreeHandle(ClickHandle handle) { snapforge_clicks_free_handle(handle.p); }

void setLogCallback(LogCallback callback) {
    snapforge_set_log_callback(reinterpret_cast<SnapforgeLogCallback>(callback));
}

} // namespace sf
