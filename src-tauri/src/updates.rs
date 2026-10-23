//! App menu item that asks the window to check for an update.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Wry};

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &application_menu(app)?,
            &file_menu(app)?,
            &edit_menu(app)?,
            &view_menu(app)?,
        ],
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "check-updates" {
            let _ = app.emit("check-updates", ());
        }
    });
    Ok(())
}

fn application_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let about = PredefinedMenuItem::about(app, Some("About KbWrite"), None)?;
    let check = MenuItem::with_id(app, "check-updates", "Check for Updates", true, None::<&str>)?;
    let sep_a = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, None)?;
    #[cfg(target_os = "macos")]
    {
        let services = PredefinedMenuItem::services(app, None)?;
        let sep_b = PredefinedMenuItem::separator(app)?;
        let hide = PredefinedMenuItem::hide(app, None)?;
        let hide_others = PredefinedMenuItem::hide_others(app, None)?;
        let sep_c = PredefinedMenuItem::separator(app)?;
        return Submenu::with_items(
            app,
            "KbWrite",
            true,
            &[
                &about,
                &check,
                &sep_a,
                &services,
                &sep_b,
                &hide,
                &hide_others,
                &sep_c,
                &quit,
            ],
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        Submenu::with_items(app, "KbWrite", true, &[&about, &check, &sep_a, &quit])
    }
}

fn file_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let close = PredefinedMenuItem::close_window(app, None)?;
    Submenu::with_items(app, "File", true, &[&close])
}

fn edit_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let undo = PredefinedMenuItem::undo(app, None)?;
    let redo = PredefinedMenuItem::redo(app, None)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = PredefinedMenuItem::copy(app, None)?;
    let paste = PredefinedMenuItem::paste(app, None)?;
    let select_all = PredefinedMenuItem::select_all(app, None)?;
    Submenu::with_items(
        app,
        "Edit",
        true,
        &[&undo, &redo, &sep, &cut, &copy, &paste, &select_all],
    )
}

fn view_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
    Submenu::with_items(app, "View", true, &[&fullscreen])
}
