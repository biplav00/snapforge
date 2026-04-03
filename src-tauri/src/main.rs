#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::sync::Mutex;

/// Holds a pre-captured screenshot taken before the overlay window appears.
pub struct PreCapturedScreen(pub Mutex<Option<String>>);

fn main() {
    // Capture the screen BEFORE creating any windows.
    // This avoids the overlay window interfering with the screenshot.
    let pre_captured = commands::capture_screen(0).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(PreCapturedScreen(Mutex::new(pre_captured)))
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
            let _overlay = tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("ScreenSnap Overlay")
            .fullscreen(true)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
