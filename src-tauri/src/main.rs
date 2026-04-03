#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod hotkeys;
mod tray;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Holds a pre-captured screenshot taken before the overlay window appears.
pub struct PreCapturedScreen(pub Mutex<Option<String>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(PreCapturedScreen(Mutex::new(None)))
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

/// Capture screen and open overlay window.
pub fn trigger_screenshot(app: &AppHandle) {
    // Capture before opening window
    let pre_captured = commands::capture_screen(0).ok();
    if let Ok(mut guard) = app.state::<PreCapturedScreen>().0.lock() {
        *guard = pre_captured;
    }

    // Close existing overlay if any
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }

    let _ = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .title("ScreenSnap Overlay")
        .fullscreen(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
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
