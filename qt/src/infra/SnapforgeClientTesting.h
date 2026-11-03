#ifndef SNAPFORGECLIENTTESTING_H
#define SNAPFORGECLIENTTESTING_H

// Control surface for the in-memory SnapforgeClient fake
// (SnapforgeClientFake.cpp). Only test targets link the fake, so this header
// is only meaningful there. Configure inputs and inspect captured calls via
// sf::test::state().

#include "SnapforgeClient.h"

#include <QByteArray>
#include <QString>
#include <functional>
#include <optional>

namespace sf::test {

struct State {
    // Error channel returned by sf::lastError().
    QString lastError;

    // Click stream — start/stop toggles clicksRunning for the probe tests.
    bool clicksRunning = false;
    bool clicksStartSucceeds = true;

    // Configurable results. nullopt == that call reports failure.
    std::optional<QString> screenshotResult = QString("/fake/shot.png");
    std::optional<QString> savePrerenderedResult = QString("/fake/pre.png");
    bool recordStartSucceeds = true;
    // recordIsAlive() result for a valid handle (false == worker exited).
    bool recordAlive = true;
    // recordOutputPath() result; nullopt == echo the requested outputPath.
    std::optional<QString> recordOutputPathOverride;
    bool recordPauseSucceeds = true;
    bool recordResumeSucceeds = true;
    bool recordStopSucceeds = true;
    QString recordStopError = QStringLiteral("fake: encoder exited");
    // Called (on the stop worker thread) at the start of recordStop; tests use
    // it to hold the finalize in flight. Must be thread-safe.
    std::function<void()> recordStopHook;
    int recordStopCalls = 0; // written on the stop worker before it posts back
    bool hasPermission = true;
    bool requestPermissionResult = true;
    QByteArray configJson;       // returned by configLoadJson()
    QByteArray savedConfigJson;  // captured from the last configSave()
    bool configSaveSucceeds = true;

    // Captured inputs for assertions.
    std::optional<RecordReq> lastRecordReq;
};

// Process-wide fake state (test binaries are single-process, single-threaded).
State &state();

// Restore defaults between tests.
void reset();

} // namespace sf::test

#endif // SNAPFORGECLIENTTESTING_H
