#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod hotkeys;
mod recording;
mod tray;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Holds a pre-captured screenshot taken before the overlay window appears.
pub struct PreCapturedScreen(pub Mutex<Option<String>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(PreCapturedScreen(Mutex::new(None)))
        .manage(recording::RecordingState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_pre_captured_screen,
            commands::capture_screen,
            commands::save_region,
            commands::save_fullscreen,
            commands::save_composited_image,
            commands::copy_composited_image,
            commands::get_config,
            commands::save_config,
            commands::open_save_folder,
            commands::reload_hotkeys,
            commands::capture_and_copy_region,
            commands::check_ffmpeg,
            commands::start_recording,
            commands::start_recording_and_show_indicator,
            commands::stop_recording,
            commands::is_recording,
        ])
        .setup(|app| {
            // Hide dock icon on macOS — tray-only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app.handle())?;
            hotkeys::register_hotkeys(app.handle())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Open transparent overlay for region selection (Lightshot-style).
pub fn trigger_screenshot(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }

    // Get screen dimensions for edge-to-edge overlay
    // Use a large size that covers any display — the OS will clamp to screen bounds
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| app.available_monitors().ok().and_then(|m| m.into_iter().next()));

    let (width, height) = if let Some(m) = monitor {
        let size = m.size();
        let scale = m.scale_factor();
        (size.width as f64 / scale, size.height as f64 / scale)
    } else {
        (3840.0, 2160.0) // fallback large size
    };

    let _ = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .title("")
        .inner_size(width, height)
        .position(0.0, 0.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .build();
}

/// Capture last region directly (no overlay).
pub fn trigger_last_region(app: &AppHandle) {
    match commands::save_last_region() {
        Ok(path) => {
            eprintln!("Saved last region to: {}", path);
        }
        Err(e) => {
            eprintln!("Last region capture failed: {}", e);
        }
    }
    let _ = app;
}

/// Toggle recording. If not recording, open overlay for region selection.
/// If recording, stop and close indicator.
pub fn trigger_recording(app: &AppHandle) {
    let state = app.state::<recording::RecordingState>();
    if state.is_recording() {
        // Stop recording
        if let Ok(mut guard) = state.handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.stop();
            }
        }
        // Close indicator window
        if let Some(window) = app.get_webview_window("recording-indicator") {
            let _ = window.close();
        }
    } else {
        // Open overlay in recording mode for region selection
        if let Some(window) = app.get_webview_window("overlay") {
            let _ = window.close();
        }

        let monitor = app
            .primary_monitor()
            .ok()
            .flatten()
            .or_else(|| app.available_monitors().ok().and_then(|m| m.into_iter().next()));

        let (width, height) = if let Some(m) = monitor {
            let size = m.size();
            let scale = m.scale_factor();
            (size.width as f64 / scale, size.height as f64 / scale)
        } else {
            (3840.0, 2160.0)
        };

        let _ = WebviewWindowBuilder::new(
            app,
            "overlay",
            WebviewUrl::App("index.html?mode=record".into()),
        )
        .title("")
        .inner_size(width, height)
        .position(0.0, 0.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .build();
    }
}

/// Open the recording indicator window after recording has started.
pub fn open_recording_indicator(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("recording-indicator") {
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(
        app,
        "recording-indicator",
        WebviewUrl::App("recording.html".into()),
    )
    .title("Recording")
    .inner_size(200.0, 50.0)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .transparent(true)
    .build();
}

/// Open preferences window (or focus if already open).
pub fn open_preferences(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("preferences") {
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(
        app,
        "preferences",
        WebviewUrl::App("preferences.html".into()),
    )
    .title("ScreenSnap Preferences")
    .inner_size(600.0, 480.0)
    .resizable(true)
    .build();
}
