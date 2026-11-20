//! System tray ("run in background"): Show / Quit menu; closing the window hides it.
//!
//! Phones have no tray: there every function here does nothing.

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri::{AppHandle, Runtime};

pub const TRAY_ID: &str = "main";
pub const MAIN_WINDOW: &str = "main";

#[cfg(mobile)]
pub fn show_main_window<R: Runtime>(_app: &AppHandle<R>) {}

#[cfg(mobile)]
pub fn apply<R: Runtime>(_app: &AppHandle<R>, _enabled: bool) -> tauri::Result<()> {
    Ok(())
}

#[cfg(mobile)]
pub fn has_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    false
}

#[cfg(desktop)]
pub fn has_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

#[cfg(desktop)]
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Create or remove the tray icon according to `enabled`.
#[cfg(desktop)]
pub fn apply<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> tauri::Result<()> {
    let exists = app.tray_by_id(TRAY_ID).is_some();
    if enabled && !exists {
        create(app)?;
    } else if !enabled && exists {
        app.remove_tray_by_id(TRAY_ID);
    }
    Ok(())
}

#[cfg(desktop)]
fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Chitach", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Chitach")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
