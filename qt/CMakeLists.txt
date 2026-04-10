cmake_minimum_required(VERSION 3.20)
project(snapforge-qt LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets Gui)

# Rust FFI static library
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target/release")
set(FFI_HEADER_DIR "${CMAKE_SOURCE_DIR}/../crates/snapforge-ffi")

add_executable(snapforge-qt
    src/main.cpp
    src/OverlayWindow.cpp
    src/AnnotationState.cpp
    src/AnnotationRenderer.cpp
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
endif()
