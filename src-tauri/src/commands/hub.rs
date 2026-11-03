//! The Hub: a project's glooms as a table of contents. Thin wrappers — the
//! logic lives in `db::topics` and `db::docs`.

use crate::commands::AppState;
use crate::db::models::{Hub, Placement, Topic};
use crate::db::{docs as docs_db, topics as topics_db};
use crate::error::AppResult;
use tauri::State;

#[tauri::command]
pub async fn hub(state: State<'_, AppState>, project_id: i64) -> AppResult<Hub> {
    Ok(Hub {
        topics: topics_db::list(&state.pool, project_id).await?,
        glooms: docs_db::in_project(&state.pool, project_id).await?,
    })
}

#[tauri::command]
pub async fn create_topic(state: State<'_, AppState>, project_id: i64, name: String) -> AppResult<Topic> {
    crate::db::projects::get(&state.pool, project_id).await?;
    topics_db::create(&state.pool, project_id, &name).await
}

#[tauri::command]
pub async fn rename_topic(state: State<'_, AppState>, id: i64, name: String) -> AppResult<Topic> {
    topics_db::rename(&state.pool, id, &name).await
}

#[tauri::command]
pub async fn delete_topic(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    topics_db::delete(&state.pool, id).await
}

#[tauri::command]
pub async fn arrange_hub(
    state: State<'_, AppState>,
    project_id: i64,
    topic_order: Vec<i64>,
    glooms: Vec<Placement>,
) -> AppResult<()> {
    topics_db::arrange(&state.pool, project_id, &topic_order, &glooms).await
}
