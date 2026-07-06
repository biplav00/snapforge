#ifndef SNAPFORGECLIENTTESTING_H
#define SNAPFORGECLIENTTESTING_H

// Control surface for the in-memory SnapforgeClient fake
// (SnapforgeClientFake.cpp). Only test targets link the fake, so this header
// is only meaningful there. Configure inputs and inspect captured calls via
// sf::test::state().

#include "SnapforgeClient.h"

#include <QByteArray>
#include <QString>
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
