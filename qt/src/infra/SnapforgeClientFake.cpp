// In-memory SnapforgeClient fake. Linked into test targets INSTEAD of
// SnapforgeClient.cpp (and instead of libsnapforge_ffi.a), so a unit under
// test exercises no real FFI, no display, and no TCC grant. Behaviour is
// driven through sf::test::state() (see SnapforgeClientTesting.h).

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

std::optional<QString> savePrerendered(const uint8_t *, std::size_t, uint32_t,
                                       uint32_t, const SaveReq &) {
    return test::state().savePrerenderedResult;
}

RecHandle recordStart(const RecordReq &req) {
    test::state().lastRecordReq = req;
    return test::state().recordStartSucceeds ? RecHandle{&kRecToken} : RecHandle{};
}
bool recordStop(RecHandle handle) {
    auto &st = test::state();
    ++st.recordStopCalls;
    // Optional blocking hook so tests can hold the stop worker mid-finalize.
    // Runs on the stop worker thread.
    // Copy first: a test may reassign the hook while a worker is parked in it.
    if (auto hook = st.recordStopHook) hook();
    if (!handle.valid()) return false;
    if (!st.recordStopSucceeds) {
        st.lastError = st.recordStopError;
        return false;
    }
    return true;
}
bool recordPause(RecHandle handle) {
    return handle.valid() && test::state().recordPauseSucceeds;
}
bool recordResume(RecHandle handle) {
    return handle.valid() && test::state().recordResumeSucceeds;
}
void recordFreeHandle(RecHandle) {}
QString recordOutputPath(RecHandle handle) {
    if (!handle.valid()) return QString();
    const auto &st = test::state();
    if (st.recordOutputPathOverride) return *st.recordOutputPathOverride;
    return st.lastRecordReq ? st.lastRecordReq->outputPath : QString();
}
int recordIsAlive(RecHandle handle) {
    if (!handle.valid()) return -1;
    return test::state().recordAlive ? 1 : 0;
}

ClickHandle clicksStart(ClickCallback, void *) {
    if (!test::state().clicksStartSucceeds) {
        test::state().lastError = QStringLiteral("fake: clicks start refused");
        return ClickHandle{};
    }
    test::state().clicksRunning = true;
    return ClickHandle{&kClickToken};
}
bool clicksStop(ClickHandle handle) {
    test::state().clicksRunning = false;
    return handle.valid();
}
void clicksFreeHandle(ClickHandle) {}

void setLogCallback(LogCallback) {}

namespace test {

State &state() {
    static State s;
    return s;
}

void reset() { state() = State{}; }

} // namespace test
} // namespace sf
