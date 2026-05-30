#ifndef SNAPFORGE_LOGCATEGORIES_H
#define SNAPFORGE_LOGCATEGORIES_H

#include <QLoggingCategory>

// Subsystem logging categories. Use these instead of bare qDebug()/qWarning()
// so every log line is tagged with the area it came from and can be filtered
// in the Logs tab. Example:
//
//     #include "LogCategories.h"
//     qCInfo(lcRecording) << "started" << path;
//     qCWarning(lcFfi, "config_save failed: %d", rc);
//
Q_DECLARE_LOGGING_CATEGORY(lcApp)        // app lifecycle / startup
Q_DECLARE_LOGGING_CATEGORY(lcUi)         // windows, widgets, user actions
Q_DECLARE_LOGGING_CATEGORY(lcRecording)  // capture + encoding pipeline
Q_DECLARE_LOGGING_CATEGORY(lcFfi)        // Rust FFI boundary
Q_DECLARE_LOGGING_CATEGORY(lcHotkeys)    // global hotkeys
Q_DECLARE_LOGGING_CATEGORY(lcHistory)    // history / saved files
Q_DECLARE_LOGGING_CATEGORY(lcAnnotation) // annotation overlay

#endif
