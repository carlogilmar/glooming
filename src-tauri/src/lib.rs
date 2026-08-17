mod commands;
mod error;
mod git;

// `db` is public for its serde models, which the integration tests construct.
pub mod db;

// Public so the integration tests in tests/ can exercise the pipeline directly.
pub mod parse;
pub mod reconcile;
pub mod seed;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let path = db::default_db_path()?;
            let pool = tauri::async_runtime::block_on(db::open_pool(&path))?;
            app.manage(AppState { pool });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::files::open_file,
            commands::files::reparse,
            commands::files::blame_file,
            commands::docs::seed_doc,
            commands::docs::create_doc,
            commands::docs::save_doc,
            commands::docs::load_doc,
            commands::docs::list_docs,
            commands::docs::delete_doc,
            commands::docs::reconcile_doc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lgtm");
}
