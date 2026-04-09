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

/// Open a fullscreen transparent overlay window on the primary monitor only.
fn open_overlays(app: &AppHandle, base_url: &str) {
    close_all_overlays(app);

    let monitors: Vec<_> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .collect();

    if let Some(monitor) = monitors.first() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let pos = monitor.position();
        let width = size.width as f64 / scale;
        let height = size.height as f64 / scale;

        let separator = if base_url.contains('?') { "&" } else { "?" };
        let url = format!("{base_url}{separator}display=0");
        let label = "overlay-0";

        let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
            .title("")
            .inner_size(width, height)
            .position(pos.x as f64 / scale, pos.y as f64 / scale)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible_on_all_workspaces(true)
            .build();
    }
}

/// Pre-capture the primary display (must run BEFORE overlay opens).
fn pre_capture_all_displays(app: &AppHandle) {
    let Ok(image) = snapforge_core::capture::capture_fullscreen(0) else {
        return;
    };
    let Ok(bytes) = snapforge_core::format::encode_image_fast(&image) else {
        return;
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    if let Ok(mut guard) = app.state::<PreCapturedScreens>().0.lock() {
        guard.clear();
        guard.insert(0, b64);
    }
}

/// Open transparent overlay for region selection on all monitors.
/// Checks permission first, captures screen, then opens overlay.
/// Runs SCK work on a background thread to avoid deadlocking the main RunLoop
/// (SCK completion handlers need the main RunLoop to be free).
pub fn trigger_screenshot(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // Try to capture directly — if it works, permission is granted.
        // Only request permission if capture fails (avoids stale preflight results
        // and unnecessary permission dialogs after app rebuilds).
        pre_capture_all_displays(&app);

        // If no screens were captured, permission is likely missing
        let has_captures = app
            .state::<PreCapturedScreens>()
            .0
            .lock()
            .is_ok_and(|g| !g.is_empty());
        if !has_captures {
            snapforge_core::capture::request_permission();
            return;
        }

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
                open_overlays(&app, &url);
                return;
            }
        }

        open_overlays(&app, "index.html");
    });
}

/// Toggle recording. If not recording, open overlay for region selection.
/// If recording, stop and close indicator.
pub fn trigger_recording(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<recording::RecordingState>();
        if state.is_recording() {
            // Stop the recording handle
            if let Ok(mut guard) = state.handle.lock() {
                if let Some(handle) = guard.take() {
                    let _ = handle.stop();
                }
            }
            close_region_outline(&app);
            close_recording_indicator(&app);

            // Add to history (skip clipboard — calling NSPasteboard from a
            // background thread is unsafe and video files can't be copied anyway)
            let path = state
                .output_path
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .unwrap_or_default();
            if !path.is_empty() {
                let _ = commands::add_to_history(path);
            }
        } else {
            // Try a quick display count to verify SCK access works
            if snapforge_core::capture::display_count() == 0 {
                snapforge_core::capture::request_permission();
                return;
            }
            open_overlays(&app, "index.html?mode=record");
        }
    });
}

/// Open the recording indicator after recording has started.
/// Switch the system tray to recording mode (shows ● Recording... and ■ Stop).
pub fn open_recording_indicator(app: &AppHandle) {
    tray::set_recording_tray(app, true);
}

/// Open a fullscreen transparent overlay window that dims everything outside
/// the recording region and shows a dashed white border around it.
/// Excluded from capture so it doesn't appear in the recording.
pub fn open_region_outline(app: &AppHandle, x: f64, y: f64, w: f64, h: f64) {
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        open_region_outline_impl(&app_clone, x, y, w, h);
    });
}

fn open_region_outline_impl(app: &AppHandle, x: f64, y: f64, w: f64, h: f64) {
    #[cfg(not(target_os = "linux"))]
    {
        close_region_outline(app);

        // Find the monitor containing the region
        let monitors: Vec<_> = app
            .available_monitors()
            .unwrap_or_default()
            .into_iter()
            .collect();

        let monitor = monitors.first();
        let (mon_x, mon_y, mon_w, mon_h) = monitor.map_or((0.0, 0.0, w, h), |m| {
            let pos = m.position();
            let size = m.size();
            let scale = m.scale_factor();
            (
                f64::from(pos.x) / scale,
                f64::from(pos.y) / scale,
                f64::from(size.width) / scale,
                f64::from(size.height) / scale,
            )
        });

        // Region coordinates relative to the monitor's top-left
        let rel_x = x - mon_x;
        let rel_y = y - mon_y;
        let url = format!("outline.html?x={rel_x}&y={rel_y}&w={w}&h={h}");

        if let Ok(window) =
            WebviewWindowBuilder::new(app, "region-outline", WebviewUrl::App(url.into()))
                .title("")
                .inner_size(mon_w, mon_h)
                .position(mon_x, mon_y)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .transparent(true)
                .skip_taskbar(true)
                .shadow(false)
                .visible_on_all_workspaces(true)
                .build()
        {
            #[cfg(target_os = "macos")]
            {
                if let Ok(ns_win) = window.ns_window() {
                    unsafe {
                        // Exclude from screen capture
                        let _: () = objc2::msg_send![
                            ns_win as *const objc2::runtime::AnyObject,
                            setSharingType: 0u64
                        ];
                        // Make click-through
                        let _: () = objc2::msg_send![
                            ns_win as *const objc2::runtime::AnyObject,
                            setIgnoresMouseEvents: true
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

    #[cfg(target_os = "linux")]
    {
        let _ = (app, x, y, w, h);
    }
}

pub fn close_region_outline(app: &AppHandle) {
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_clone.get_webview_window("region-outline") {
            let _ = window.close();
        }
    });
}

/// Restore the default system tray menu.
pub fn close_recording_indicator(app: &AppHandle) {
    tray::set_recording_tray(app, false);
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
