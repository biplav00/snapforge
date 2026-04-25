cmake_minimum_required(VERSION 3.20)
project(snapforge-qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Default to Release if the user didn't pick a build type.
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets Gui Concurrent)

# Rust FFI static library. Match the Rust profile to the C++ build type so
# Debug C++ links against Debug Rust (and Release against Release).
string(TOLOWER "${CMAKE_BUILD_TYPE}" _rust_profile_dir)
if(_rust_profile_dir STREQUAL "debug")
    set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target/debug")
else()
    set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target/release")
endif()
set(FFI_HEADER_DIR "${CMAKE_SOURCE_DIR}/../crates/snapforge-ffi")

set(MACOSX_BUNDLE_BUNDLE_NAME "Snapforge")
set(MACOSX_BUNDLE_GUI_IDENTIFIER "com.snapforge.app")
set(MACOSX_BUNDLE_BUNDLE_VERSION "1.0.0")
set(MACOSX_BUNDLE_SHORT_VERSION_STRING "1.0.0")
set(MACOSX_BUNDLE_INFO_STRING "Snapforge Screenshot Tool")
set(MACOSX_BUNDLE_ICON_FILE "AppIcon.icns")

# App icon — packaged into the bundle's Resources folder.
set(APP_ICON_MACOSX "${CMAKE_SOURCE_DIR}/resources/AppIcon.icns")
set_source_files_properties("${APP_ICON_MACOSX}" PROPERTIES
    MACOSX_PACKAGE_LOCATION "Resources"
)

# Tray PNG (128x128 @2x) embedded as a Qt resource for the status item.
qt_add_resources(SNAPFORGE_QRC_FILES
    "${CMAKE_SOURCE_DIR}/resources/snapforge.qrc"
)

set(SNAPFORGE_SOURCES
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
    src/AnnotationRenderer.cpp
    src/AnnotationCanvas.cpp
    src/AnnotationToolbar.cpp
    src/RecordingManager.cpp
    src/HistoryWindow.cpp
    src/PreferencesWindow.cpp
)

if(APPLE)
    list(APPEND SNAPFORGE_SOURCES
        src/WorkspaceSleepObserver.mm
        src/SpaceChangeObserver.mm)
    # The .mm files use ARC's __bridge_retained / __bridge_transfer; enable ARC.
    set_source_files_properties(src/WorkspaceSleepObserver.mm
        PROPERTIES COMPILE_FLAGS "-fobjc-arc")
    set_source_files_properties(src/SpaceChangeObserver.mm
        PROPERTIES COMPILE_FLAGS "-fobjc-arc")
endif()

add_executable(snapforge-qt MACOSX_BUNDLE
    ${SNAPFORGE_SOURCES}
    ${APP_ICON_MACOSX}
    ${SNAPFORGE_QRC_FILES}
)

target_include_directories(snapforge-qt PRIVATE
    ${FFI_HEADER_DIR}
)

target_link_libraries(snapforge-qt PRIVATE
    Qt6::Widgets
    Qt6::Gui
    Qt6::Concurrent
    "${RUST_TARGET_DIR}/libsnapforge_ffi.a"
)

# macOS frameworks required by snapforge-core
if(APPLE)
    target_link_libraries(snapforge-qt PRIVATE
        "-framework ScreenCaptureKit"
        "-framework CoreGraphics"
        "-framework CoreMedia"
        "-framework CoreFoundation"
        "-framework Foundation"
        "-framework AppKit"
        "-framework IOKit"
        "-framework IOSurface"
        "-framework Security"
        "-framework Carbon"
    )

    # Custom Info.plist properties for accessory app (no dock icon)
    set_target_properties(snapforge-qt PROPERTIES
        MACOSX_BUNDLE_INFO_PLIST "${CMAKE_SOURCE_DIR}/Info.plist.in"
    )
endif()

# macdeployqt bundles Qt frameworks into the .app
find_program(MACDEPLOYQT macdeployqt HINTS "${Qt6_DIR}/../../../bin")

if(APPLE AND MACDEPLOYQT)
    # Every link of snapforge-qt resets the binary's LC_LOAD_DYLIB entries back
    # to the absolute Homebrew paths, which makes the bundle load two copies of
    # Qt at runtime. Re-run macdeployqt after each build so @executable_path
    # rewrites stick.
    add_custom_command(TARGET snapforge-qt POST_BUILD
        COMMAND ${MACDEPLOYQT} "$<TARGET_BUNDLE_DIR:snapforge-qt>"
        COMMENT "Rewriting bundle framework paths via macdeployqt..."
        VERBATIM
    )
endif()

add_custom_target(dmg
    DEPENDS snapforge-qt
    COMMAND ${MACDEPLOYQT} "$<TARGET_BUNDLE_DIR:snapforge-qt>" -dmg
    COMMENT "Creating DMG..."
)
