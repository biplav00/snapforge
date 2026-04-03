use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let screenshot_item = MenuItem::with_id(app, "screenshot", "Screenshot", true, None::<&str>)?;
    let last_region_item =
        MenuItem::with_id(app, "last_region", "Capture Last Region", true, None::<&str>)?;
    let record_item = MenuItem::with_id(app, "record", "Record Screen", true, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let open_folder_item =
        MenuItem::with_id(app, "open_folder", "Open Save Folder", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let preferences_item =
        MenuItem::with_id(app, "preferences", "Preferences", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &screenshot_item,
            &last_region_item,
            &record_item,
            &separator1,
            &open_folder_item,
            &separator2,
            &preferences_item,
            &quit_item,
        ],
    )?;

    // Decode embedded PNG to RGBA for tray icon
    let icon_png = include_bytes!("../icons/32x32.png");
    let img = image::load_from_memory(icon_png).expect("failed to decode tray icon");
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let icon = tauri::image::Image::new_owned(rgba.into_raw(), w, h);

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "screenshot" => {
                crate::trigger_screenshot(app);
            }
            "last_region" => {
                crate::trigger_last_region(app);
            }
            "record" => {
                crate::trigger_recording(app);
            }
            "open_folder" => {
                let _ = crate::commands::open_save_folder();
            }
            "preferences" => {
                crate::open_preferences(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
