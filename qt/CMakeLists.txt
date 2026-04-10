cmake_minimum_required(VERSION 3.20)
project(snapforge-qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets Gui)

# Rust FFI static library
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target/release")
set(FFI_HEADER_DIR "${CMAKE_SOURCE_DIR}/../crates/snapforge-ffi")

set(MACOSX_BUNDLE_BUNDLE_NAME "Snapforge")
set(MACOSX_BUNDLE_GUI_IDENTIFIER "com.snapforge.app")
set(MACOSX_BUNDLE_BUNDLE_VERSION "1.0.0")
set(MACOSX_BUNDLE_SHORT_VERSION_STRING "1.0.0")
set(MACOSX_BUNDLE_INFO_STRING "Snapforge Screenshot Tool")
set(MACOSX_BUNDLE_ICON_FILE "AppIcon")

add_executable(snapforge-qt MACOSX_BUNDLE
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

target_include_directories(snapforge-qt PRIVATE
    ${FFI_HEADER_DIR}
)

target_link_libraries(snapforge-qt PRIVATE
    Qt6::Widgets
    Qt6::Gui
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

add_custom_target(dmg
    DEPENDS snapforge-qt
    COMMAND ${MACDEPLOYQT} "$<TARGET_BUNDLE_DIR:snapforge-qt>" -dmg
    COMMENT "Creating DMG..."
)
