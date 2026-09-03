// `pub` for the same reason `db` is: `tests/pipeline.rs` runs the real chain
// rather than a copy of it.
pub mod commands;
mod error;
pub mod git;

// `db` is public for its serde models, which the integration tests construct.
pub mod db;

// Public so the integration tests in tests/ can exercise the pipeline directly.
pub mod import;
pub mod parse;
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
            commands::files::branch_of,
            commands::files::blame_file,
            commands::docs::seed_doc,
            commands::docs::create_doc,
            commands::docs::save_doc,
            commands::docs::load_doc,
            commands::docs::list_docs,
            commands::docs::delete_doc,
            commands::docs::block_for,
            commands::docs::open_reading,
            commands::docs::add_doc_file,
            commands::docs::remove_doc_file,
            commands::docs::export_note,
            commands::docs::gloom_template,
            commands::docs::preview_import,
            commands::docs::import_gloom,
            commands::projects::open_project,
            commands::projects::recent_projects,
            commands::projects::forget_project,
            commands::projects::project_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lgtm");
}
