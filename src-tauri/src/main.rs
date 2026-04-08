#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod hotkeys;
mod recording;
mod tray;

use base64::Engine;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Holds pre-captured screenshots for each display (display index -> base64 PNG).
pub struct PreCapturedScreens(pub Mutex<HashMap<usize, String>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PreCapturedScreens(Mutex::new(HashMap::new())))
        .manage(recording::RecordingState::new())
        .invoke_handler(tauri::generate_handler![
            commands::show_toast,
            commands::get_display_count,
            commands::get_pre_captured_screen,
            commands::capture_screen,
            commands::save_region,
            commands::save_fullscreen,
            commands::save_composited_image,
            commands::copy_composited_image,
            commands::copy_file_to_clipboard,
            commands::capture_and_copy_region,
            commands::get_config,
            commands::save_config,
            commands::open_save_folder,
            commands::reload_hotkeys,
            commands::save_last_region_to_config,
            commands::get_history,
            commands::add_to_history,
            commands::open_file_in_folder,
            commands::clear_history,
            commands::check_ffmpeg,
            commands::start_recording,
            commands::start_recording_and_show_indicator,
            commands::stop_recording,
            commands::is_recording,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create_tray(app.handle())?;
            hotkeys::register_hotkeys(app.handle())?;

            // Pre-warm webview to cache JS bundle for instant overlay startup
            prewarm_overlay(app.handle());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Keep app running in the system tray when windows close,
            // but allow explicit quit (from tray menu or app.exit()).
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    // No exit code = triggered by last window closing. Prevent it.
                    api.prevent_exit();
                }
                // If code is Some(_), it's an explicit exit (tray Quit). Let it through.
            }
        });
}

/// Close all existing overlay windows.
fn close_all_overlays(app: &AppHandle) {
    let labels: Vec<String> = app
        .webview_windows()
        .keys()
        .filter(|l| l.starts_with("overlay"))
        .cloned()
        .collect();
    for label in labels {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.close();
        }
    }
}

/// Open a fullscreen transparent overlay window on each monitor.
fn open_overlays(app: &AppHandle, base_url: &str) {
    close_all_overlays(app);

    let monitors: Vec<_> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .collect();

    for (i, monitor) in monitors.iter().enumerate() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let pos = monitor.position();
        let width = size.width as f64 / scale;
        let height = size.height as f64 / scale;

        let separator = if base_url.contains('?') { "&" } else { "?" };
        let url = format!("{base_url}{separator}display={i}");
        let label = format!("overlay-{i}");

        let _ = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title("")
            .inner_size(width, height)
            .position(pos.x as f64 / scale, pos.y as f64 / scale)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .build();
    }
}

/// Pre-warm the webview by creating and immediately closing a throwaway overlay.
/// This caches the JS bundle so subsequent overlays open much faster.
fn prewarm_overlay(app: &AppHandle) {
    let label = "prewarm";
    if let Ok(_w) = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("")
        .inner_size(1.0, 1.0)
        .position(-9999.0, -9999.0)
        .transparent(true)
        .decorations(false)
        .visible(false)
        .build()
    {
        // Close after webview loads (give it 500ms to cache resources)
        let app_clone = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Some(w) = app_clone.get_webview_window(label) {
                let _ = w.close();
            }
        });
    }
}

/// Pre-capture all displays synchronously (must run BEFORE overlay opens).
/// Uses parallel threads + fast PNG encoding for speed.
fn pre_capture_all_displays(app: &AppHandle) {
    let count = snapforge_core::capture::display_count();

    let results: Vec<(usize, String)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..count)
            .map(|i| {
                s.spawn(move || -> Option<(usize, String)> {
                    let image = snapforge_core::capture::capture_fullscreen(i).ok()?;
                    let bytes = snapforge_core::format::encode_image_fast(&image).ok()?;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    Some((i, b64))
                })
            })
            .collect();

        handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect()
    });

    if let Ok(mut guard) = app.state::<PreCapturedScreens>().0.lock() {
        guard.clear();
        for (i, b64) in results {
            guard.insert(i, b64);
        }
    }
}

/// Open transparent overlay for region selection on all monitors.
/// Checks permission first, captures screen, then opens overlay.
pub fn trigger_screenshot(app: &AppHandle) {
    // Check screen capture permission before doing anything
    if !snapforge_core::capture::has_permission() {
        // Request triggers the macOS permission dialog — don't capture or open overlay yet
        snapforge_core::capture::request_permission();
        return;
    }

    pre_capture_all_displays(app);

    let config = snapforge_core::config::AppConfig::load().unwrap_or_default();
    if config.remember_last_region {
        if let Some(last) = config.last_region {
            let monitors: Vec<_> = app
                .available_monitors()
                .unwrap_or_default()
                .into_iter()
                .collect();
            let idx = last.display.min(monitors.len().saturating_sub(1));
            let dpr = monitors.get(idx).map_or(2.0, tauri::Monitor::scale_factor);
            let x = last.rect.x as f64 / dpr;
            let y = last.rect.y as f64 / dpr;
            let w = last.rect.width as f64 / dpr;
            let h = last.rect.height as f64 / dpr;
            let url = format!(
                "index.html?lastRegion={},{},{},{}",
                x as i32, y as i32, w as u32, h as u32
            );
            open_overlays(app, &url);
            return;
        }
    }

    open_overlays(app, "index.html");
}

/// Toggle recording. If not recording, open overlay for region selection.
/// If recording, stop and close indicator.
pub fn trigger_recording(app: &AppHandle) {
    let state = app.state::<recording::RecordingState>();
    if state.is_recording() {
        if let Ok(mut guard) = state.handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.stop();
            }
        }
        close_recording_indicator(app);
    } else {
        if !snapforge_core::capture::has_permission() {
            snapforge_core::capture::request_permission();
            return;
        }
        open_overlays(app, "index.html?mode=record");
    }
}

/// Open the recording indicator after recording has started.
/// On macOS/Windows: floating window excluded from screen capture.
/// On Linux: no floating window (no reliable capture exclusion API); uses tray menu instead.
pub fn open_recording_indicator(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    {
        // On Linux, update tray to show recording status with a stop option.
        tray::set_recording_tray(app, true);
    }

    #[cfg(not(target_os = "linux"))]
    {
        if let Some(window) = app.get_webview_window("recording-indicator") {
            let _ = window.set_focus();
            return;
        }

        if let Ok(window) = WebviewWindowBuilder::new(
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
        .build()
        {
            #[cfg(target_os = "macos")]
            {
                if let Ok(ns_win) = window.ns_window() {
                    unsafe {
                        let _: () = objc2::msg_send![
                            ns_win as *const objc2::runtime::AnyObject,
                            setSharingType: 0u64
                        ];
                    }
                }
            }

            #[cfg(target_os = "windows")]
            {
                if let Ok(hwnd) = window.hwnd() {
                    unsafe {
                        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(
                            hwnd.0 as _,
                            0x11,
                        );
                    }
                }
            }

            let _ = &window;
        }
    }
}

/// Close the recording indicator (window or tray state).
pub fn close_recording_indicator(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    {
        tray::set_recording_tray(app, false);
    }

    #[cfg(not(target_os = "linux"))]
    {
        if let Some(window) = app.get_webview_window("recording-indicator") {
            let _ = window.close();
        }
    }
}

/// Open history window (or focus if already open).
pub fn open_history(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("history") {
        let _ = window.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(app, "history", WebviewUrl::App("history.html".into()))
        .title("Screenshot History")
        .inner_size(700.0, 500.0)
        .resizable(true)
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
    .title("Snapforge Preferences")
    .inner_size(600.0, 480.0)
    .resizable(true)
    .build();
}
