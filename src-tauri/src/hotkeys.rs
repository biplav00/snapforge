// src-tauri/src/hotkeys.rs
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn register_hotkeys(app: &AppHandle) -> tauri::Result<()> {
    let config = screen_core::config::AppConfig::load().unwrap_or_default();

    // Register screenshot hotkey
    if let Ok(shortcut) = config.hotkey_bindings.screenshot.parse::<Shortcut>() {
        let app_handle = app.clone();
        let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::trigger_screenshot(&app_handle);
            }
        });
    }

    // Register last-region hotkey
    if let Ok(shortcut) = config.hotkey_bindings.capture_last_region.parse::<Shortcut>() {
        let app_handle = app.clone();
        let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::trigger_last_region(&app_handle);
            }
        });
    }

    Ok(())
}

/// Unregister all hotkeys and re-register from current config.
pub fn reload_hotkeys(app: &AppHandle) -> tauri::Result<()> {
    let _ = app.global_shortcut().unregister_all();
    register_hotkeys(app)
}
