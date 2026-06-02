cmake_minimum_required(VERSION 3.20)

# Universal (arm64 + x86_64) build, opt-in only. Enable with
# -DSNAPFORGE_UNIVERSAL=ON or the SNAPFORGE_UNIVERSAL=1 env var (qt/build.sh
# forwards the flag). When OFF (the default) nothing here runs and the build
# stays host-arch — byte-for-byte the existing behaviour. Must be set before
# project() so CMAKE_OSX_ARCHITECTURES is picked up by the C/CXX compiler
# probes. The matching universal Rust staticlib + fat ffmpeg dylibs are
# produced by qt/build.sh and packaging/macos/bundle-ffmpeg.sh respectively.
option(SNAPFORGE_UNIVERSAL "Build a universal arm64 + x86_64 binary" OFF)
if(NOT SNAPFORGE_UNIVERSAL AND DEFINED ENV{SNAPFORGE_UNIVERSAL}
   AND "$ENV{SNAPFORGE_UNIVERSAL}" STREQUAL "1")
    set(SNAPFORGE_UNIVERSAL ON)
endif()
if(SNAPFORGE_UNIVERSAL)
    set(CMAKE_OSX_ARCHITECTURES "arm64;x86_64" CACHE STRING
        "Universal binary architectures" FORCE)
    message(STATUS "SNAPFORGE_UNIVERSAL: building for arm64;x86_64")
endif()

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

set(SNAPFORGE_VERSION "1.3.11")
set(MACOSX_BUNDLE_BUNDLE_NAME "Snapforge")
set(MACOSX_BUNDLE_GUI_IDENTIFIER "com.snapforge.app")
set(MACOSX_BUNDLE_BUNDLE_VERSION "1.3.12")
set(MACOSX_BUNDLE_SHORT_VERSION_STRING "1.3.12")
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
    src/app/main.cpp
    src/infra/Logger.cpp
    src/infra/LogCategories.cpp
    src/infra/Shortcuts.cpp
    src/ui/overlay/OverlayWindow.cpp
    src/ui/annotation/AnnotationState.cpp
    src/ui/annotation/AnnotationRenderer.cpp
    src/ui/annotation/AnnotationCanvas.cpp
    src/ui/annotation/AnnotationToolbar.cpp
    src/capture/RecordingManager.cpp
    src/ui/windows/HistoryWindow.cpp
    src/ui/windows/PreferencesWindow.cpp
    src/ui/overlay/ClickIndicatorOverlay.cpp
    src/ui/tray/TrayIcon.cpp
    src/controllers/RecordingController.cpp
    src/controllers/ClickTap.cpp
)

if(APPLE)
    list(APPEND SNAPFORGE_SOURCES
        src/platform/macos/WorkspaceSleepObserver.mm
        src/platform/macos/SpaceChangeObserver.mm
        src/ui/overlay/ClickIndicatorOverlayMac.mm)
    # The .mm files use ARC's __bridge_retained / __bridge_transfer; enable ARC.
    set_source_files_properties(src/platform/macos/WorkspaceSleepObserver.mm
        PROPERTIES COMPILE_FLAGS "-fobjc-arc")
    set_source_files_properties(src/platform/macos/SpaceChangeObserver.mm
        PROPERTIES COMPILE_FLAGS "-fobjc-arc")
endif()

add_executable(snapforge-qt MACOSX_BUNDLE
    ${SNAPFORGE_SOURCES}
    ${APP_ICON_MACOSX}
    ${SNAPFORGE_QRC_FILES}
)

# Produce Snapforge.app / Contents/MacOS/Snapforge so macOS Privacy
# settings, the dock, and TCC all show "Snapforge" instead of the CMake
# target name.
set_target_properties(snapforge-qt PROPERTIES OUTPUT_NAME "Snapforge")

target_include_directories(snapforge-qt PRIVATE
    ${FFI_HEADER_DIR}
    src/app
    src/ui/windows
    src/ui/overlay
    src/ui/annotation
    src/ui/tray
    src/controllers
    src/capture
    src/infra
    src/platform/macos
)

target_compile_definitions(snapforge-qt PRIVATE
    SNAPFORGE_VERSION="${SNAPFORGE_VERSION}"
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

    # Bundle ffmpeg + its Homebrew dylibs into the .app so shipped builds run
    # on machines without Homebrew. Runs after macdeployqt (it walks
    # Contents/MacOS/) and before codesign (signature must cover ffmpeg).
    add_custom_command(TARGET snapforge-qt POST_BUILD
        COMMAND "${CMAKE_SOURCE_DIR}/../packaging/macos/bundle-ffmpeg.sh"
                "$<TARGET_BUNDLE_DIR:snapforge-qt>"
        COMMENT "Bundling ffmpeg and dylibs..."
        VERBATIM
    )

    # Ad-hoc sign with a stable identifier. Without this, every CI build
    # gets a fresh ad-hoc signature, so macOS TCC treats each build as a
    # different app and the user has to re-grant Screen Recording. Pinning
    # the identifier to com.snapforge.app keeps TCC's grant stable across
    # rebuilds. Runs after macdeployqt + ffmpeg bundling so the framework
    # re-bundling doesn't invalidate the signature.
    add_custom_command(TARGET snapforge-qt POST_BUILD
        COMMAND codesign --force --deep --sign - --identifier com.snapforge.app
                "$<TARGET_BUNDLE_DIR:snapforge-qt>"
        COMMENT "Ad-hoc signing with stable identifier..."
        VERBATIM
    )
endif()

# DMG packaging lives in packaging/macos/build_dmg.sh (single source of truth,
# used by both local builds and CI). Don't add a competing cmake dmg target —
# it drifts from the script (no /Applications symlink, no versioned volname).

# ---------------------------------------------------------------------------
# Tests (QTest)
#
# Built only under BUILD_TESTING (CTest's default-ON variable), so the normal
# `cmake --build` of the app never compiles or links the test executables.
# Each test links the specific logic units it exercises (not the whole app) so
# the suite stays headless: no widgets, no tray, no capture. The Rust staticlib
# + macOS frameworks are linked the same way the app does because the units
# under test call into the FFI (config load/save, record start/stop).
# ---------------------------------------------------------------------------
include(CTest) # defines BUILD_TESTING (default ON) and calls enable_testing()

if(BUILD_TESTING)
    # Qt6::Test is only needed for the suite, so resolve it here rather than in
    # the top-level find_package — the app build never pulls it in.
    find_package(Qt6 REQUIRED COMPONENTS Test)

    # Frameworks + staticlib that the FFI-touching units need at link time.
    # Factored out so each test target lists the same dependency block.
    set(SNAPFORGE_TEST_FFI_LIBS "${RUST_TARGET_DIR}/libsnapforge_ffi.a")
    if(APPLE)
        list(APPEND SNAPFORGE_TEST_FFI_LIBS
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
    endif()

    # Include dirs the units under test expect (FFI header + their own folders).
    set(SNAPFORGE_TEST_INCLUDES
        ${FFI_HEADER_DIR}
        src/infra
        src/capture
    )

    # name = test target/ctest name, remaining args = extra .cpp sources to
    # compile into that test alongside the test driver itself.
    function(snapforge_add_test name)
        add_executable(${name} ${ARGN})
        target_include_directories(${name} PRIVATE ${SNAPFORGE_TEST_INCLUDES})
        target_link_libraries(${name} PRIVATE
            Qt6::Test
            Qt6::Core
            Qt6::Gui
            ${SNAPFORGE_TEST_FFI_LIBS}
        )
        add_test(NAME ${name} COMMAND ${name})
    endfunction()

    # Pure chord parsing/formatting (the dynamic-hotkeys feature). GUILESS.
    snapforge_add_test(tst_shortcuts
        tests/tst_shortcuts.cpp
        src/infra/Shortcuts.cpp
    )

    # RecordingManager state machine + timing + prefs parsing. GUILESS; FFI
    # calls are exercised only on paths that fail safely without capture.
    snapforge_add_test(tst_recordingmanager
        tests/tst_recordingmanager.cpp
        src/capture/RecordingManager.cpp
    )

    # Integration-ish smoke: construct the manager, round-trip config-derived
    # prefs, and a display-gated capture probe that QSKIPs when
    # SNAPFORGE_REQUIRE_DISPLAY is unset (mirrors the Rust integration gating).
    snapforge_add_test(tst_smoke
        tests/tst_smoke.cpp
        src/capture/RecordingManager.cpp
        src/infra/Shortcuts.cpp
    )
endif()
