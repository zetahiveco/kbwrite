mod ai;
mod commands;
mod graph;
mod settings;
mod vault;

use std::sync::Mutex;

pub fn run() {
    let settings = settings::Settings::load();
    let (vault, error) = commands::open_vault(&settings.vault_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState {
            settings: Mutex::new(settings),
            vault: Mutex::new(commands::VaultSlot { vault, error }),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::reload_vault,
            commands::read_note,
            commands::write_note,
            commands::create_file,
            commands::create_folder,
            commands::rename_path,
            commands::delete_path,
            commands::save_settings,
            commands::ai_complete,
            commands::graph,
            commands::open_privacy_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KbWrite");
}
