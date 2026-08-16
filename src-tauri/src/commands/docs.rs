//! Doc CRUD plus seeding. Thin wrappers — the logic lives in `db::docs`,
//! `seed` and `reconcile`.

use crate::commands::files::sha256;
use crate::commands::AppState;
use crate::db::docs as docs_db;
use crate::db::models::{Doc, DocSummary};
use crate::error::AppResult;
use crate::parse::Outline;
use crate::{reconcile, seed};
use std::path::Path;
use tauri::State;

/// Build the starter markdown for a parsed file without saving it — lets the UI
/// show a preview before the doc exists.
#[tauri::command]
pub fn seed_doc(outline: Outline) -> String {
    seed::seed_markdown(&outline)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_doc(
    state: State<'_, AppState>,
    path: String,
    lang: String,
    title: String,
    branch: Option<String>,
    markdown: String,
    source: String,
) -> AppResult<Doc> {
    let filename = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let sha = sha256(&source);

    docs_db::create(
        &state.pool,
        &path,
        &filename,
        &lang,
        &title,
        branch.as_deref(),
        &markdown,
        &source,
        &sha,
    )
    .await
}

#[tauri::command]
pub async fn save_doc(
    state: State<'_, AppState>,
    id: i64,
    markdown: Option<String>,
    title: Option<String>,
    branch: Option<String>,
    label: Option<String>,
) -> AppResult<Doc> {
    docs_db::update(
        &state.pool,
        id,
        markdown.as_deref(),
        title.as_deref(),
        branch.as_deref(),
        label.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn load_doc(state: State<'_, AppState>, id: i64) -> AppResult<Doc> {
    docs_db::get(&state.pool, id).await
}

#[tauri::command]
pub async fn list_docs(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<DocSummary>> {
    docs_db::list(&state.pool, query.as_deref(), limit.unwrap_or(100)).await
}

#[tauri::command]
pub async fn delete_doc(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    docs_db::delete(&state.pool, id).await
}

/// Merge a doc with freshly-parsed source: your prose survives, new functions
/// are appended, vanished ones are struck through. Then re-snapshot, so the
/// doc stops reading as stale.
#[tauri::command]
pub async fn reconcile_doc(
    state: State<'_, AppState>,
    id: i64,
    outline: Outline,
    source: String,
) -> AppResult<Doc> {
    let doc = docs_db::get(&state.pool, id).await?;
    let merged = reconcile::reconcile_markdown(&doc.markdown, &outline);
    let sha = sha256(&source);
    docs_db::update_source(&state.pool, id, &source, &sha, &merged).await
}
