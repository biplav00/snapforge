# Qt Native Frontend — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal Qt prototype that replaces the Tauri/Svelte overlay with a native C++ Qt frontend, using the existing Rust `snapforge-core` via C FFI, to validate that hotkey-to-overlay latency drops from ~800ms to <100ms.

**Architecture:** The Rust `snapforge-core` crate is compiled into a static library (`libsnapforge_ffi.a`) via a thin C FFI wrapper crate. A C++ Qt6 application links against this library and provides the native UI: system tray, global hotkey, transparent fullscreen overlay with region selection, and save/copy functionality.

**Tech Stack:** Rust (existing core), C (FFI layer), C++17 (Qt frontend), Qt6 (Widgets, Gui), CMake (build system)

---

## File Structure

```
crates/snapforge-ffi/
├── Cargo.toml              # FFI crate config (staticlib)
├── src/
│   └── lib.rs              # extern "C" wrappers over snapforge-core
├── snapforge_ffi.h         # C header (consumed by Qt app)

qt/
├── CMakeLists.txt          # Qt6 + link to libsnapforge_ffi.a
├── src/
│   ├── main.cpp            # QApplication entry, tray, hotkey
│   ├── OverlayWindow.h     # Transparent fullscreen overlay widget
│   ├── OverlayWindow.cpp   # Region selection, dim mask, QPainter
│   └── snapforge_ffi.h     # Symlink or copy of FFI header
└── resources/
    └── icon.png            # Tray icon
```

---

### Task 1: Install Qt6 and CMake

**Files:** None (system setup)

- [ ] **Step 1: Install Qt6 and CMake via Homebrew**

```bash
brew install qt cmake
```

- [ ] **Step 2: Verify installation**

```bash
cmake --version
qmake6 --version
```

Expected: cmake 3.x and Qt 6.x version output.

- [ ] **Step 3: Find Qt6 prefix for CMake**

```bash
brew --prefix qt
```

Expected: Something like `/opt/homebrew/opt/qt`. Note this path — CMake needs it.

---

### Task 2: Create the FFI crate

**Files:**
- Create: `crates/snapforge-ffi/Cargo.toml`
- Create: `crates/snapforge-ffi/src/lib.rs`
- Create: `crates/snapforge-ffi/snapforge_ffi.h`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add FFI crate to workspace**

In the root `Cargo.toml`, add `"crates/snapforge-ffi"` to the workspace members:

```toml
[workspace]
resolver = "2"
members = [
    "crates/snapforge-core",
    "crates/snapforge-ffi",
    "cli",
    "src-tauri",
]
```

- [ ] **Step 2: Create FFI crate Cargo.toml**

Create `crates/snapforge-ffi/Cargo.toml`:

```toml
[package]
name = "snapforge-ffi"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["staticlib"]

[dependencies]
snapforge-core = { path = "../snapforge-core" }
image = { version = "0.25", default-features = false, features = ["png"] }

[lints]
workspace = true
```

- [ ] **Step 3: Create FFI wrapper implementation**

Create `crates/snapforge-ffi/src/lib.rs`:

```rust
//! C FFI wrappers over snapforge-core for the Qt frontend.
//!
//! Convention:
//! - Return 0 on success, -1 on error.
//! - Caller-owned buffers are allocated here and freed via snapforge_free_buffer.
//! - String out-params are allocated here and freed via snapforge_free_string.

use std::ffi::{c_char, c_int, CStr, CString};
use std::path::Path;
use std::ptr;

use snapforge_core::capture;
use snapforge_core::clipboard;
use snapforge_core::format;
use snapforge_core::types::{CaptureFormat, Rect};

/// Captured image data returned to C callers.
/// The caller must free `data` via `snapforge_free_buffer`.
#[repr(C)]
pub struct CapturedImage {
    pub data: *mut u8,
    pub len: usize,
    pub width: u32,
    pub height: u32,
}

/// Capture the full screen for a given display index.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
/// Returns: width > 0 on success, width == 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_capture_fullscreen(display: u32) -> CapturedImage {
    let result = capture::capture_fullscreen(display as usize);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let mut raw = image.into_raw();
            let data = raw.as_mut_ptr();
            let len = raw.len();
            std::mem::forget(raw);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
    }
}

/// Capture a region of the screen.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_capture_region(
    display: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> CapturedImage {
    let region = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    let result = capture::capture_region(display as usize, region);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let mut raw = image.into_raw();
            let data = raw.as_mut_ptr();
            let len = raw.len();
            std::mem::forget(raw);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
    }
}

/// Free a pixel buffer returned by snapforge_capture_*.
/// SAFETY: `data` must have been returned by a snapforge_capture_* function.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_free_buffer(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(data, len, len);
        }
    }
}

/// Save RGBA pixel data to a file.
/// `path` is a null-terminated UTF-8 string.
/// `fmt`: 0 = PNG, 1 = JPG, 2 = WebP.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_save_image(
    data: *const u8,
    width: u32,
    height: u32,
    path: *const c_char,
    fmt: u32,
    quality: u8,
) -> c_int {
    if data.is_null() || path.is_null() {
        return -1;
    }

    let path_str = unsafe { CStr::from_ptr(path) };
    let path = match path_str.to_str() {
        Ok(s) => Path::new(s),
        Err(_) => return -1,
    };

    let format = match fmt {
        0 => CaptureFormat::Png,
        1 => CaptureFormat::Jpg,
        2 => CaptureFormat::WebP,
        _ => return -1,
    };

    let len = (width as usize) * (height as usize) * 4;
    let slice = unsafe { std::slice::from_raw_parts(data, len) };

    let image = match image::RgbaImage::from_raw(width, height, slice.to_vec()) {
        Some(img) => img,
        None => return -1,
    };

    match format::save_image(&image, path, format, quality) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Copy RGBA pixel data to the system clipboard.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_copy_to_clipboard(data: *const u8, width: u32, height: u32) -> c_int {
    if data.is_null() {
        return -1;
    }

    let len = (width as usize) * (height as usize) * 4;
    let slice = unsafe { std::slice::from_raw_parts(data, len) };

    let image = match image::RgbaImage::from_raw(width, height, slice.to_vec()) {
        Some(img) => img,
        None => return -1,
    };

    match clipboard::copy_image_to_clipboard(&image) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Check if screen capture permission is granted.
/// Returns 1 if granted, 0 if not.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_has_permission() -> c_int {
    if capture::has_permission() { 1 } else { 0 }
}

/// Request screen capture permission.
/// Returns 1 if granted, 0 if not.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_request_permission() -> c_int {
    if capture::request_permission() { 1 } else { 0 }
}

/// Get the number of available displays.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_display_count() -> u32 {
    capture::display_count() as u32
}

/// Get the DPI scale factor of the primary display.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_display_scale_factor() -> f64 {
    capture::display_scale_factor()
}

/// Get the default save directory path (e.g. ~/Pictures/Snapforge/).
/// Returns a heap-allocated null-terminated string. Caller must free via snapforge_free_string.
/// Returns NULL on error.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_default_save_path() -> *mut c_char {
    let config = snapforge_core::config::AppConfig::load().unwrap_or_default();
    let dir = config.save_directory();
    match CString::new(dir.to_string_lossy().as_ref()) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string returned by snapforge_* functions.
#[unsafe(no_mangle)]
pub extern "C" fn snapforge_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}
```

- [ ] **Step 4: Create the C header file**

Create `crates/snapforge-ffi/snapforge_ffi.h`:

```c
#ifndef SNAPFORGE_FFI_H
#define SNAPFORGE_FFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint8_t *data;
    size_t len;
    uint32_t width;
    uint32_t height;
} CapturedImage;

/* Capture fullscreen. Caller must free result.data via snapforge_free_buffer. */
CapturedImage snapforge_capture_fullscreen(uint32_t display);

/* Capture a region. Caller must free result.data via snapforge_free_buffer. */
CapturedImage snapforge_capture_region(uint32_t display, int32_t x, int32_t y, uint32_t w, uint32_t h);

/* Free a pixel buffer returned by snapforge_capture_*. */
void snapforge_free_buffer(uint8_t *data, size_t len);

/* Save RGBA data to file. fmt: 0=PNG, 1=JPG, 2=WebP. Returns 0 on success. */
int snapforge_save_image(const uint8_t *data, uint32_t width, uint32_t height,
                         const char *path, uint32_t fmt, uint8_t quality);

/* Copy RGBA data to clipboard. Returns 0 on success. */
int snapforge_copy_to_clipboard(const uint8_t *data, uint32_t width, uint32_t height);

/* Check screen capture permission. Returns 1 if granted. */
int snapforge_has_permission(void);

/* Request screen capture permission. Returns 1 if granted. */
int snapforge_request_permission(void);

/* Get number of displays. */
uint32_t snapforge_display_count(void);

/* Get primary display DPI scale factor. */
double snapforge_display_scale_factor(void);

/* Get default save directory. Caller must free via snapforge_free_string. */
char *snapforge_default_save_path(void);

/* Free a string returned by snapforge_*. */
void snapforge_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif /* SNAPFORGE_FFI_H */
```

- [ ] **Step 5: Build the FFI crate and verify it compiles**

```bash
cd crates/snapforge-ffi && cargo build --release
```

Expected: Successful build. Verify `target/release/libsnapforge_ffi.a` exists:

```bash
ls -la ../../target/release/libsnapforge_ffi.a
```

- [ ] **Step 6: Commit**

```bash
git add crates/snapforge-ffi/ Cargo.toml
git commit -m "feat: add snapforge-ffi crate with C FFI wrappers over core"
```

---

### Task 3: Create the Qt CMake project

**Files:**
- Create: `qt/CMakeLists.txt`
- Create: `qt/src/main.cpp` (minimal stub)

- [ ] **Step 1: Create the Qt project directory**

```bash
mkdir -p qt/src qt/resources
```

- [ ] **Step 2: Create CMakeLists.txt**

Create `qt/CMakeLists.txt`:

```cmake
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
    )
endif()
```

- [ ] **Step 3: Create a minimal main.cpp stub**

Create `qt/src/main.cpp`:

```cpp
#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QMessageBox>
#include "snapforge_ffi.h"

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);

    // Verify FFI link works
    uint32_t displays = snapforge_display_count();

    // System tray
    QSystemTrayIcon tray;
    tray.setIcon(QIcon::fromTheme("camera-photo"));
    tray.setToolTip(QString("Snapforge (%1 displays)").arg(displays));

    QMenu menu;
    menu.addAction("Quit", &app, &QApplication::quit);
    tray.setContextMenu(&menu);
    tray.show();

    return app.exec();
}
```

- [ ] **Step 4: Create empty OverlayWindow files (stubs for CMake)**

Create `qt/src/OverlayWindow.h`:

```cpp
#ifndef OVERLAYWINDOW_H
#define OVERLAYWINDOW_H

#include <QWidget>

class OverlayWindow : public QWidget {
    Q_OBJECT

public:
    explicit OverlayWindow(QWidget *parent = nullptr);
    void activate();

signals:
    void regionCaptured(int x, int y, int w, int h);
    void cancelled();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;

private:
    QPoint m_startPos;
    QPoint m_endPos;
    bool m_drawing = false;
    bool m_hasRegion = false;
    QImage m_screenshot;

    QRect selectedRect() const;
};

#endif // OVERLAYWINDOW_H
```

Create `qt/src/OverlayWindow.cpp`:

```cpp
#include "OverlayWindow.h"
#include <QPainter>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QScreen>
#include <QGuiApplication>
#include "snapforge_ffi.h"

OverlayWindow::OverlayWindow(QWidget *parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool)
{
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_ShowWithoutActivating, false);
    setCursor(Qt::CrossCursor);
    setMouseTracking(true);
}

void OverlayWindow::activate() {
    // Pre-capture screen before showing overlay
    CapturedImage img = snapforge_capture_fullscreen(0);
    if (img.data && img.width > 0) {
        m_screenshot = QImage(img.data, img.width, img.height, img.width * 4,
                              QImage::Format_RGBA8888).copy();
        snapforge_free_buffer(img.data, img.len);
    }

    // Size to primary screen
    QScreen *screen = QGuiApplication::primaryScreen();
    if (screen) {
        setGeometry(screen->geometry());
    }

    m_drawing = false;
    m_hasRegion = false;
    m_startPos = QPoint();
    m_endPos = QPoint();

    showFullScreen();
    activateWindow();
    raise();
}

QRect OverlayWindow::selectedRect() const {
    return QRect(m_startPos, m_endPos).normalized();
}

void OverlayWindow::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);

    // Draw pre-captured screenshot as background (optional — shows what will be captured)
    if (!m_screenshot.isNull()) {
        p.drawImage(0, 0, m_screenshot.scaled(size(), Qt::IgnoreAspectRatio, Qt::FastTransformation));
    }

    if (m_drawing || m_hasRegion) {
        QRect sel = selectedRect();

        // Dim everything outside the selection
        QPainterPath fullPath;
        fullPath.addRect(rect());
        QPainterPath selPath;
        selPath.addRect(sel);
        QPainterPath dimPath = fullPath.subtracted(selPath);
        p.fillPath(dimPath, QColor(0, 0, 0, 100));

        // Selection border — marching ants effect
        QPen whitePen(Qt::white, 1, Qt::DashLine);
        p.setPen(whitePen);
        p.drawRect(sel);

        QPen darkPen(QColor(0, 0, 0, 150), 1, Qt::DashLine);
        darkPen.setDashOffset(4);
        p.setPen(darkPen);
        p.drawRect(sel);

        // Dimension label
        QString label = QString("%1 × %2").arg(sel.width()).arg(sel.height());
        QFont font("monospace", 11);
        p.setFont(font);
        QFontMetrics fm(font);
        QRect labelRect = fm.boundingRect(label);
        int lx = sel.x() + sel.width() / 2 - labelRect.width() / 2 - 4;
        int ly = sel.y() - labelRect.height() - 8;
        p.fillRect(lx - 2, ly - 1, labelRect.width() + 8, labelRect.height() + 4,
                   QColor(0, 0, 0, 180));
        p.setPen(Qt::white);
        p.drawText(lx + 2, ly + labelRect.height() - 2, label);

        // Resize handles (8 points)
        if (m_hasRegion && !m_drawing) {
            p.setPen(QColor(0, 0, 0, 128));
            p.setBrush(Qt::white);
            int hs = 4; // half-size
            QPoint handles[] = {
                sel.topLeft(), sel.topRight(), sel.bottomLeft(), sel.bottomRight(),
                QPoint(sel.center().x(), sel.top()),
                QPoint(sel.center().x(), sel.bottom()),
                QPoint(sel.left(), sel.center().y()),
                QPoint(sel.right(), sel.center().y()),
            };
            for (const auto &pt : handles) {
                p.drawRect(pt.x() - hs, pt.y() - hs, hs * 2, hs * 2);
            }
        }
    } else {
        // No selection yet — light dim over everything
        p.fillRect(rect(), QColor(0, 0, 0, 40));
    }
}

void OverlayWindow::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        m_startPos = event->pos();
        m_endPos = event->pos();
        m_drawing = true;
        m_hasRegion = false;
        update();
    }
}

void OverlayWindow::mouseMoveEvent(QMouseEvent *event) {
    if (m_drawing) {
        m_endPos = event->pos();
        update();
    }
}

void OverlayWindow::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton && m_drawing) {
        m_drawing = false;
        QRect sel = selectedRect();
        if (sel.width() > 5 && sel.height() > 5) {
            m_hasRegion = true;
            update();
        }
    }
}

void OverlayWindow::keyPressEvent(QKeyEvent *event) {
    if (event->key() == Qt::Key_Escape) {
        m_hasRegion = false;
        m_drawing = false;
        hide();
        emit cancelled();
    } else if (event->key() == Qt::Key_Return && m_hasRegion) {
        QRect sel = selectedRect();
        hide();

        // Get DPI scale factor for pixel-accurate capture
        double dpr = snapforge_display_scale_factor();
        int px = static_cast<int>(sel.x() * dpr);
        int py = static_cast<int>(sel.y() * dpr);
        int pw = static_cast<int>(sel.width() * dpr);
        int ph = static_cast<int>(sel.height() * dpr);

        emit regionCaptured(px, py, pw, ph);
    } else if ((event->modifiers() & Qt::ControlModifier) && event->key() == Qt::Key_C && m_hasRegion) {
        // Cmd+C / Ctrl+C — copy to clipboard
        QRect sel = selectedRect();
        double dpr = snapforge_display_scale_factor();
        int px = static_cast<int>(sel.x() * dpr);
        int py = static_cast<int>(sel.y() * dpr);
        int pw = static_cast<int>(sel.width() * dpr);
        int ph = static_cast<int>(sel.height() * dpr);

        CapturedImage img = snapforge_capture_region(0, px, py, pw, ph);
        if (img.data && img.width > 0) {
            snapforge_copy_to_clipboard(img.data, img.width, img.height);
            snapforge_free_buffer(img.data, img.len);
        }

        m_hasRegion = false;
        m_drawing = false;
        hide();
        emit cancelled();
    }
}
```

- [ ] **Step 5: Build the Qt app**

```bash
cd qt
cmake -B build -DCMAKE_PREFIX_PATH="$(brew --prefix qt)"
cmake --build build
```

Expected: Successful build producing `qt/build/snapforge-qt`.

- [ ] **Step 6: Run and verify tray icon appears**

```bash
./qt/build/snapforge-qt
```

Expected: A system tray icon appears. Right-click shows "Quit" menu. The tooltip shows display count from the Rust FFI.

- [ ] **Step 7: Commit**

```bash
git add qt/
git commit -m "feat: add Qt frontend with tray icon, overlay, and region selection"
```

---

### Task 4: Add global hotkey and save functionality

**Files:**
- Modify: `qt/src/main.cpp`
- Modify: `qt/CMakeLists.txt` (if needed)

- [ ] **Step 1: Update main.cpp with hotkey and save wiring**

Replace `qt/src/main.cpp` with:

```cpp
#include <QApplication>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QDir>
#include <QDateTime>
#include <QShortcut>
#include <QTimer>
#include "OverlayWindow.h"
#include "snapforge_ffi.h"

#ifdef Q_OS_MAC
#include <Carbon/Carbon.h>

// macOS global hotkey callback
static OverlayWindow *g_overlay = nullptr;

OSStatus hotkeyHandler(EventHandlerCallRef, EventRef, void *) {
    if (g_overlay) {
        // Use QTimer to ensure we're on the Qt event loop
        QTimer::singleShot(0, g_overlay, &OverlayWindow::activate);
    }
    return noErr;
}

void registerGlobalHotkey() {
    EventHotKeyRef hotKeyRef;
    EventHotKeyID hotKeyID;
    hotKeyID.signature = 'SNPF';
    hotKeyID.id = 1;

    EventTypeSpec eventType;
    eventType.eventClass = kEventClassKeyboard;
    eventType.eventKind = kEventHotKeyPressed;

    InstallApplicationEventHandler(&hotkeyHandler, 1, &eventType, nullptr, nullptr);

    // Cmd+Shift+S = kVK_ANSI_S (0x01), modifiers: cmdKey | shiftKey
    RegisterEventHotKey(0x01, cmdKey | shiftKey, hotKeyID,
                        GetApplicationEventTarget(), 0, &hotKeyRef);
}
#endif

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName("Snapforge");
    app.setQuitOnLastWindowClosed(false);

#ifdef Q_OS_MAC
    // Hide from dock (accessory app)
    app.setAttribute(Qt::AA_DontShowIconsInMenus);
#endif

    // Request permission if needed
    if (!snapforge_has_permission()) {
        snapforge_request_permission();
    }

    // Pre-create overlay (kept hidden — shown on hotkey)
    OverlayWindow overlay;
    g_overlay = &overlay;

    // Handle region capture — save to file
    QObject::connect(&overlay, &OverlayWindow::regionCaptured,
                     [](int x, int y, int w, int h) {
        CapturedImage img = snapforge_capture_region(0, x, y, w, h);
        if (!img.data || img.width == 0) return;

        // Generate save path
        char *saveDir = snapforge_default_save_path();
        QString dir = saveDir ? QString::fromUtf8(saveDir) : QDir::homePath() + "/Pictures/Snapforge";
        if (saveDir) snapforge_free_string(saveDir);

        QDir().mkpath(dir);
        QString timestamp = QDateTime::currentDateTime().toString("yyyy-MM-dd_HH-mm-ss");
        QString path = dir + "/screenshot_" + timestamp + ".png";

        int result = snapforge_save_image(img.data, img.width, img.height,
                                          path.toUtf8().constData(), 0, 90);
        snapforge_free_buffer(img.data, img.len);

        if (result == 0) {
            qDebug("Saved: %s", qPrintable(path));
        }
    });

    // System tray
    QSystemTrayIcon tray;
    tray.setIcon(QIcon::fromTheme("camera-photo"));
    tray.setToolTip("Snapforge");

    QMenu menu;
    menu.addAction("Screenshot (Cmd+Shift+S)", &overlay, &OverlayWindow::activate);
    menu.addSeparator();
    menu.addAction("Quit", &app, &QApplication::quit);
    tray.setContextMenu(&menu);
    tray.show();

#ifdef Q_OS_MAC
    registerGlobalHotkey();
#endif

    return app.exec();
}
```

- [ ] **Step 2: Update CMakeLists.txt to link Carbon framework (for global hotkeys)**

Add to the `if(APPLE)` block in `qt/CMakeLists.txt`:

```cmake
    target_link_libraries(snapforge-qt PRIVATE
        "-framework Carbon"
    )
```

The full `if(APPLE)` block should now include Carbon alongside the other frameworks.

- [ ] **Step 3: Rebuild and test**

```bash
cd qt && cmake --build build
./build/snapforge-qt
```

Expected:
1. Tray icon appears
2. Press Cmd+Shift+S → fullscreen transparent overlay appears instantly
3. Draw a region → resize handles and dimension label appear
4. Press Enter → region is captured and saved to ~/Pictures/Snapforge/
5. Press Cmd+C → region is copied to clipboard
6. Press Escape → overlay closes

- [ ] **Step 4: Measure hotkey-to-overlay latency**

Add timing to `OverlayWindow::activate()`. In `OverlayWindow.cpp`, at the start of `activate()`:

```cpp
#include <QElapsedTimer>

void OverlayWindow::activate() {
    QElapsedTimer timer;
    timer.start();
```

At the end of `activate()`, after `raise()`:

```cpp
    qDebug("Overlay activated in %lld ms", timer.elapsed());
}
```

Rebuild and test. Expected: output like `Overlay activated in 40 ms` or less.

- [ ] **Step 5: Commit**

```bash
git add qt/
git commit -m "feat: add global hotkey (Cmd+Shift+S) and save/copy functionality"
```

---

### Task 5: Build script for one-command build

**Files:**
- Create: `qt/build.sh`

- [ ] **Step 1: Create the build script**

Create `qt/build.sh`:

```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Building Rust FFI library ==="
cd "$ROOT_DIR"
cargo build --release -p snapforge-ffi

echo "=== Configuring Qt project ==="
cd "$SCRIPT_DIR"
QT_PREFIX="$(brew --prefix qt 2>/dev/null || echo /opt/homebrew/opt/qt)"
cmake -B build -DCMAKE_PREFIX_PATH="$QT_PREFIX"

echo "=== Building Qt app ==="
cmake --build build

echo "=== Done ==="
echo "Binary: $SCRIPT_DIR/build/snapforge-qt"
```

- [ ] **Step 2: Make it executable and test**

```bash
chmod +x qt/build.sh
./qt/build.sh
```

Expected: Builds both the Rust FFI library and the Qt app in sequence. Prints the binary path.

- [ ] **Step 3: Commit**

```bash
git add qt/build.sh
git commit -m "feat: add one-command build script for Qt frontend"
```

---

### Task 6: Verify performance and compare

**Files:** None (testing only)

- [ ] **Step 1: Run the Qt prototype and measure**

```bash
./qt/build/snapforge-qt
```

Press Cmd+Shift+S multiple times. Check the console output for timing.

Expected: `Overlay activated in XX ms` where XX < 100.

- [ ] **Step 2: Compare with Tauri version**

Run the existing Tauri app:

```bash
cargo tauri dev
```

Press Cmd+Shift+S and subjectively compare the two.

- [ ] **Step 3: Document results**

Note the measured latency in the terminal. If <100ms, the prototype validates the approach.
