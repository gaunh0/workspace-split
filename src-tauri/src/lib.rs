mod commands;
mod db;
mod error;
mod models;
mod paths;
mod state;
mod watcher;

use std::path::PathBuf;

use directories::BaseDirs;
use tauri::Manager;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Folder under `%APPDATA%` that holds the database; matches `identifier` in
/// `tauri.conf.json`.
const DATA_DIR_NAME: &str = "com.gaunh0.spacebox";
const DB_FILE_NAME: &str = "workspace.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let database_path = database_path()?;
            let connection = db::open(&database_path)?;
            app.manage(AppState::new(connection));

            if let Err(error) = watcher::restart(app.handle()) {
                // Watching is best-effort; the app still works with manual checks.
                log::warn!("file watcher could not start: {error}");
            }

            log::info!("database ready at {}", database_path.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_space,
            commands::list_spaces,
            commands::delete_space,
            commands::add_resource,
            commands::list_resources,
            commands::search_resources,
            commands::update_resource,
            commands::delete_resource,
            commands::check_missing_files,
            commands::reveal_in_explorer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Tauri application");
}

fn database_path() -> AppResult<PathBuf> {
    // On Windows `BaseDirs::data_dir` is `%APPDATA%` (Roaming).
    let base_dirs = BaseDirs::new().ok_or(AppError::DataDirUnavailable)?;
    Ok(base_dirs.data_dir().join(DATA_DIR_NAME).join(DB_FILE_NAME))
}
