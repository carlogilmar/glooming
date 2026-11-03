//! Gloom CRUD, and taking a note out of the app. Thin wrappers — the logic
//! lives in `db::docs`.

use crate::commands::files::normalize_path;
use crate::commands::AppState;
use crate::db::models::{Doc, DocSummary};
use crate::db::docs as docs_db;
use crate::error::AppResult;
use std::path::Path;
use tauri::State;

/// What a new gloom starts as: one line saying what the page is for.
///
/// Found again *structurally* by the frontend — the last paragraph, nothing but
/// emphasis — so the arrival nudge points at it, and the moment you type a word
/// it stops matching. Nothing has to remember whether a gloom is "fresh".
pub const INVITATION: &str = "_The code is on the left — write what you make of it here. \
Press `/` while editing to reference any function in this project; once your prose \
names one, `▷ Gloom` will walk the code in the order you wrote it._\n";

/// Start a gloom in a project. Unnamed: the band selects the name for you, so
/// naming it costs a sentence and skipping it costs one key — and asking before
/// you have read anything would ask at the one moment you cannot answer.
#[tauri::command]
pub async fn create_doc(state: State<'_, AppState>, project_id: i64) -> AppResult<Doc> {
    crate::db::projects::get(&state.pool, project_id).await?;
    docs_db::create(&state.pool, project_id, "", INVITATION).await
}

#[tauri::command]
pub async fn save_doc(
    state: State<'_, AppState>,
    id: i64,
    markdown: Option<String>,
    title: Option<String>,
    label: Option<String>,
) -> AppResult<Doc> {
    docs_db::update(
        &state.pool,
        id,
        markdown.as_deref(),
        title.as_deref(),
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
    project_id: i64,
    query: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<DocSummary>> {
    docs_db::list(&state.pool, project_id, query.as_deref(), limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn delete_doc(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    docs_db::delete(&state.pool, id).await
}

/// Write a note's markdown to a path the user picked in the save dialog.
///
/// A single-purpose command rather than `tauri-plugin-fs`, deliberately. The
/// plugin would put a general "write files" capability into the app — with a
/// scope to get wrong — to serve one gesture whose destination the user has
/// already named in a native dialog. This writes that text to that path and can
/// do nothing else.
///
/// It is also the only place lgtm writes anything outside its own database. The
/// rule it does not break: **it never writes into a repository it read from.**
/// The path comes from the dialog, the content is the note, and no source file
/// is touched.
#[tauri::command]
pub fn export_note(path: String, markdown: String) -> AppResult<String> {
    let path = normalize_path(&path);
    let p = Path::new(&path);

    // A missing parent is the one failure worth naming: the dialog can return a
    // path under a directory that has since gone, and `io error: No such file or
    // directory` does not say which part is missing.
    if let Some(dir) = p.parent() {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            return Err(crate::error::AppError::BadInput(format!(
                "{} does not exist",
                dir.display()
            )));
        }
    }

    std::fs::write(p, markdown)?;
    Ok(path)
}

#[cfg(test)]
mod export_tests {
    use super::export_note;

    #[test]
    fn writes_the_markdown_verbatim() {
        let dir = std::env::temp_dir().join(format!("lgtm-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("accounts.md");

        // Exactly what is in the note, byte for byte. The whole point of the
        // markdown being the data is that it travels unchanged — no front
        // matter, no generated title, nothing added on the way out.
        let md = "Then `MyApp.Billing.charge/2` builds the invoice.\n\n- one\n- two\n";
        let out = export_note(file.to_string_lossy().into(), md.into()).expect("writes");

        assert_eq!(std::fs::read_to_string(&out).expect("reads"), md);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_directory_says_which_one() {
        let missing = std::env::temp_dir().join("lgtm-no-such-dir-9f3a").join("x.md");
        let err = export_note(missing.to_string_lossy().into(), "x".into())
            .expect_err("refuses");
        let msg = err.to_string();
        assert!(msg.contains("lgtm-no-such-dir-9f3a"), "got: {msg}");
        assert!(msg.contains("does not exist"), "got: {msg}");
    }

    /// Overwriting is the dialog's decision, not ours: the user has already been
    /// asked, and refusing here would make "save again" fail.
    #[test]
    fn writing_twice_replaces_the_file() {
        let dir = std::env::temp_dir().join(format!("lgtm-export2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("note.md");
        let p: String = file.to_string_lossy().into();

        export_note(p.clone(), "first".into()).expect("writes");
        export_note(p.clone(), "second".into()).expect("writes again");
        assert_eq!(std::fs::read_to_string(&file).expect("reads"), "second");
        std::fs::remove_dir_all(&dir).ok();
    }
}
