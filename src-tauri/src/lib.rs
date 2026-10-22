mod commands;
mod graph;
mod mcp;
mod media;
mod settings;
mod vault;

use std::sync::{Arc, Mutex};

pub fn run() {
    let settings = settings::Settings::load();
    let (vault, error) = commands::open_vault(&settings.vault_path);

    tauri::Builder::default()
        .register_uri_scheme_protocol("vault", |ctx, request| {
            media::respond(ctx.app_handle(), &request)
        })
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState {
            settings: Arc::new(Mutex::new(settings)),
            vault: Arc::new(Mutex::new(commands::VaultSlot { vault, error })),
            mcp: Mutex::new(None),
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
            commands::mcp_status,
            commands::mcp_start,
            commands::mcp_stop,
            commands::save_media,
            commands::read_media_text,
            commands::open_media,
            commands::graph,
            commands::open_privacy_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KbWrite");
}
