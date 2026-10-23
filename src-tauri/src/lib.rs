mod commands;
mod graph;
mod mcp;
mod media;
mod plugins;
mod settings;
mod updates;
mod vault;

use std::sync::{Arc, Mutex};

pub fn run() {
    let settings = settings::Settings::load();
    let (vault, error) = commands::open_vault(&settings.vault_path);
    let dialogs = Arc::new(plugins::DialogHub::default());

    tauri::Builder::default()
        .register_uri_scheme_protocol("vault", |ctx, request| {
            media::respond(ctx.app_handle(), &request)
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(commands::AppState {
            settings: Arc::new(Mutex::new(settings)),
            vault: Arc::new(Mutex::new(commands::VaultSlot { vault, error })),
            mcp: Mutex::new(None),
            plugins: Mutex::new(plugins::host(dialogs.clone())),
            dialogs,
        })
        .setup(|app| {
            updates::install(app.handle())?;
            plugins::attach(app.handle());
            Ok(())
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
            plugins::list_plugins,
            plugins::install_plugin_folder,
            plugins::install_plugin_github,
            plugins::set_plugin_enabled,
            plugins::remove_plugin,
            plugins::run_plugin_command,
            plugins::resolve_plugin_dialog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KbWrite");
}
