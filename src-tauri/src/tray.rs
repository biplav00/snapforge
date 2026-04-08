use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

#[cfg(target_os = "linux")]
use tauri::tray::TrayIconId;

const TRAY_ID: &str = "main-tray";

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_default_menu(app)?;

    let icon_png = include_bytes!("../icons/tray-icon.png");
    let img = image::load_from_memory(icon_png)
        .map_err(|e| tauri::Error::AssetNotFound(format!("tray icon decode failed: {}", e)))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let icon = tauri::image::Image::new_owned(rgba.into_raw(), w, h);

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .build(app)?;

    Ok(())
}

fn build_default_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let screenshot_item = MenuItem::with_id(app, "screenshot", "Screenshot", true, None::<&str>)?;
    let record_item = MenuItem::with_id(app, "record", "Record Screen", true, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let open_folder_item =
        MenuItem::with_id(app, "open_folder", "Open Save Folder", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let preferences_item =
        MenuItem::with_id(app, "preferences", "Preferences", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &screenshot_item,
            &record_item,
            &separator1,
            &open_folder_item,
            &history_item,
            &separator2,
            &preferences_item,
            &quit_item,
        ],
    )
}

#[cfg(target_os = "linux")]
fn build_recording_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let recording_label = MenuItem::with_id(
        app,
        "recording_label",
        "● Recording...",
        false,
        None::<&str>,
    )?;
    let stop_item = MenuItem::with_id(
        app,
        "stop_recording",
        "■ Stop Recording",
        true,
        None::<&str>,
    )?;

    Menu::with_items(app, &[&recording_label, &stop_item])
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "screenshot" => {
            crate::trigger_screenshot(app);
        }
        "record" | "stop_recording" => {
            crate::trigger_recording(app);
        }
        "open_folder" => {
            let _ = crate::commands::open_save_folder();
        }
        "history" => {
            crate::open_history(app);
        }
        "preferences" => {
            crate::open_preferences(app);
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

/// On Linux: swap the tray menu between default and recording mode.
/// On other platforms this is a no-op (they use the floating indicator window).
#[cfg(target_os = "linux")]
pub fn set_recording_tray(app: &AppHandle, recording: bool) {
    let tray_id = TrayIconId::new(TRAY_ID);
    if let Some(tray) = app.tray_by_id(&tray_id) {
        let menu = if recording {
            build_recording_menu(app)
        } else {
            build_default_menu(app)
        };
        if let Ok(menu) = menu {
            let _ = tray.set_menu(Some(menu));
        }
        let tooltip = if recording {
            "Snapforge — Recording..."
        } else {
            "Snapforge"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}
