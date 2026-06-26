// In-memory SnapforgeClient fake. Linked into test targets INSTEAD of
// SnapforgeClient.cpp (and instead of libsnapforge_ffi.a), so a unit under
// test exercises no real FFI, no display, and no TCC grant. Behaviour is
// driven through sf::test::state() / fireClick() (see SnapforgeClientTesting.h).

#include "SnapforgeClient.h"
#include "SnapforgeClientTesting.h"

namespace sf {
namespace {
// Distinct non-null sentinels so handle.valid() is true and a record handle
// is never equal to a click handle.
int kRecToken = 0;
int kClickToken = 0;
} // namespace

QString lastError() { return test::state().lastError; }
int lastErrorCode() { return test::state().lastErrorCode; }

std::optional<QImage> captureFullscreen(uint32_t) {
    QImage img(2, 2, QImage::Format_RGBA8888);
    img.fill(Qt::red);
    return img;
}
std::optional<QImage> captureRegion(uint32_t, const Region &region) {
    const int w = region.width > 0 ? static_cast<int>(region.width) : 1;
    const int h = region.height > 0 ? static_cast<int>(region.height) : 1;
    QImage img(w, h, QImage::Format_RGBA8888);
    img.fill(Qt::red);
    return img;
}

bool hasPermission() { return test::state().hasPermission; }
bool requestPermission() { return test::state().requestPermissionResult; }
int displayAtPoint(int32_t, int32_t) { return 0; }
double displayScaleFactor() { return 2.0; }
QString defaultSavePath() { return QStringLiteral("/fake/Snapforge"); }

QByteArray historyListJson() {
    return QByteArrayLiteral("[]");
}
bool historyDelete(const QString &) { return true; }
bool historyClear() { return true; }
bool isIncompleteMp4(const QString &) { return false; }

QByteArray configLoadJson() { return test::state().configJson; }
bool configSave(const QByteArray &json) {
    test::state().savedConfigJson = json;
    return test::state().configSaveSucceeds;
}

std::optional<QString> takeScreenshot(const ScreenshotReq &req) {
    test::state().lastScreenshotReq = req;
    return test::state().screenshotResult;
}
std::optional<QString> savePrerendered(const uint8_t *, std::size_t, uint32_t,
                                       uint32_t, const SaveReq &) {
    return test::state().savePrerenderedResult;
}

RecHandle recordStart(const RecordReq &req) {
    test::state().lastRecordReq = req;
    return test::state().recordStartSucceeds ? RecHandle{&kRecToken} : RecHandle{};
}
bool recordStop(RecHandle handle) { return handle.valid(); }
bool recordPause(RecHandle handle) { return handle.valid(); }
bool recordResume(RecHandle handle) { return handle.valid(); }
void recordFreeHandle(RecHandle) {}

ClickHandle clicksStart(ClickCallback callback, void *userData) {
    if (!test::state().clicksStartSucceeds) {
        test::state().lastError = QStringLiteral("fake: clicks start refused");
        return ClickHandle{};
    }
    test::state().clickCb = callback;
    test::state().clickUserData = userData;
    test::state().clicksRunning = true;
    return ClickHandle{&kClickToken};
}
bool clicksStop(ClickHandle handle) {
    test::state().clicksRunning = false;
    return handle.valid();
}
void clicksFreeHandle(ClickHandle) {
    test::state().clickCb = nullptr;
    test::state().clickUserData = nullptr;
}

void setLogCallback(LogCallback) {}

namespace test {

State &state() {
    static State s;
    return s;
}

void reset() { state() = State{}; }

void fireClick(double x, double y, bool rightClick) {
    State &s = state();
    if (s.clickCb) s.clickCb(x, y, rightClick ? 1 : 0, s.clickUserData);
}

} // namespace test
} // namespace sf
