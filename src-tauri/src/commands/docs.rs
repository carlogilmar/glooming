//! Doc CRUD plus seeding. Thin wrappers — the logic lives in `db::docs`,
//! `seed` and `reconcile`.

use crate::commands::files::{normalize_path, sha256};
use crate::commands::AppState;
use crate::db::models::{Doc, DocSummary, Reading, ReadingFile};
use crate::db::{doc_files as files_db, docs as docs_db};
use crate::error::AppResult;
use crate::parse::Outline;
use crate::{reconcile, seed};
use std::path::Path;
use tauri::State;

/// Build the starter markdown for a parsed file without saving it.
///
/// Git history is looked up here rather than passed in, so the frontend never
/// has to sequence two calls just to seed a doc.
#[tauri::command]
pub fn seed_doc(path: String, outline: Outline, source: String) -> String {
    let history = crate::git::history(Path::new(&path)).unwrap_or_default();
    let filename = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    seed::seed_markdown(&outline, &source, &history, &filename)
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

    let doc = docs_db::create(
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
    .await?;

    // Every reading starts as a one-file reading, and the origin is a row here
    // like any other file — so nothing downstream has to special-case "the
    // first one" when walking the set.
    files_db::add(
        &state.pool, doc.id, &path, &filename, &lang, &source, &sha,
    )
    .await?;

    Ok(doc)
}

/// Read one file of a reading as the UI needs it.
///
/// Disk wins when the file is readable, because you are reviewing the code as
/// it is now; the snapshot is the fallback, which is what lets a reading survive
/// one of its files being deleted or moved.
fn read_one(f: &crate::db::models::DocFile, origin: &str) -> ReadingFile {
    let p = Path::new(&f.path);
    let disk = std::fs::read_to_string(p).ok();
    let missing = disk.is_none();
    let source = disk.unwrap_or_else(|| f.source.clone());
    let source_sha = sha256(&source);
    let lang = crate::parse::lang_for_path(&f.path).map(str::to_string);

    // A parse failure must not stop you reading the file — you just get no
    // outline, so its functions never join the reference vocabulary.
    let outline = match &lang {
        Some(l) => crate::parse::parse(&source, l).ok(),
        None => None,
    };

    ReadingFile {
        filename: f.filename.clone(),
        // Staleness is a property of *this* file, which is the whole reason
        // there is a row per file rather than one snapshot per doc.
        stale: !missing && source_sha != f.source_sha,
        missing,
        snapshot_sha: f.source_sha.clone(),
        has_git: crate::git::repo_root(p).is_some(),
        branch: crate::git::current_branch(p),
        origin: f.path == origin,
        path: f.path.clone(),
        source,
        source_sha,
        lang,
        outline,
    }
}

async fn build_reading(state: &State<'_, AppState>, id: i64) -> AppResult<Reading> {
    let doc = docs_db::get(&state.pool, id).await?;
    let rows = files_db::list(&state.pool, id).await?;
    let files = rows.iter().map(|f| read_one(f, &doc.path)).collect();
    Ok(Reading { doc, files })
}

/// The `lgtm:surface` block for an already-parsed file, generated on demand.
///
/// Blocks that are renderable but not seeded — surface, deps, treemap — are
/// inserted from the `/` menu instead, so you get the one you want where you want
/// it rather than three you scroll past every time.
///
/// In Rust even though the frontend already holds the outline, because **the sort
/// order has to live in exactly one place**. Surface was once sorted in both
/// `seed.rs` and the renderer and the two disagreed: Rust orders by
/// `(name, arity)` giving `get_user/1, get_user!/1`, while JS `localeCompare` on
/// the whole signature reorders punctuation. A second generator would be a third
/// chance to drift.
#[tauri::command]
pub fn block_for(kind: String, path: String, outline: Outline) -> AppResult<String> {
    let bad = |m: String| crate::error::AppError::BadInput(m);

    // `stats` needs more than the outline — the line counts come from the source
    // and the history from git — and it is the one block every kind can produce,
    // so it is answered before anything looks for a module.
    if kind == "stats" {
        let p = Path::new(&path);
        let source = std::fs::read_to_string(p)?;
        let history = crate::git::history(p).unwrap_or_default();
        return Ok(match (&outline.config, &outline.tests, outline.modules.first()) {
            (Some(config), _, _) => seed::config_stats_block(config, &source, &history),
            (_, Some(tests), _) => seed::test_stats_block(tests, &source, &history),
            (_, _, Some(module)) => seed::stats_block(module, &source, &history),
            _ => return Err(bad("nothing recognised in this file".into())),
        });
    }

    // The rest are per kind: a config has no functions to lay out, a module has no
    // describes. Asking for the wrong one says so rather than returning an empty
    // fence, because an empty block reads as broken.
    match kind.as_str() {
        "settings" => outline
            .config
            .as_ref()
            .map(|c| {
                let name = Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                seed::settings_block(c, &name)
            })
            .ok_or_else(|| bad("this file is not a config script".into())),

        "tests" => outline
            .tests
            .as_ref()
            .map(seed::tests_block)
            .ok_or_else(|| bad("this file is not a test suite".into())),

        "surface" | "deps" | "treemap" => {
            let module = outline
                .modules
                .first()
                .ok_or_else(|| bad("no module in this file".into()))?;
            match kind.as_str() {
                "surface" => Ok(seed::surface_block(module)),
                "treemap" => Ok(seed::treemap_block(module)),
                "deps" if !module.deps.is_empty() => Ok(seed::deps_block(module)),
                _ => Err(bad("this module reaches nothing outside itself".into())),
            }
        }

        other => Err(bad(format!("no block called {other}"))),
    }
}

/// A whole reading in one payload: the doc, and every file it covers with its
/// source, outline and staleness.
#[tauri::command]
pub async fn open_reading(state: State<'_, AppState>, id: i64) -> AppResult<Reading> {
    build_reading(&state, id).await
}

/// Add a file to a reading.
///
/// **Nothing is seeded.** The note is yours from the first file onward; what an
/// added file contributes is source to read and functions to reference, which
/// is exactly what a review needs and no more.
#[tauri::command]
pub async fn add_doc_file(state: State<'_, AppState>, id: i64, path: String) -> AppResult<Reading> {
    let path = normalize_path(&path);
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(crate::error::AppError::NotFound(format!(
            "no file at {path}"
        )));
    }
    let source = std::fs::read_to_string(p)?;
    let sha = sha256(&source);
    let filename = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let lang = crate::parse::lang_for_path(&path).unwrap_or("text");

    files_db::add(&state.pool, id, &path, &filename, lang, &source, &sha).await?;
    build_reading(&state, id).await
}

/// Drop a file you opened by accident. The prose is untouched — a reference to
/// something no longer in the reading simply reads as dangling, which is the
/// same signal a deleted function gets.
#[tauri::command]
pub async fn remove_doc_file(
    state: State<'_, AppState>,
    id: i64,
    path: String,
) -> AppResult<Reading> {
    let doc = docs_db::get(&state.pool, id).await?;
    files_db::remove(&state.pool, id, &path, &doc.path).await?;
    build_reading(&state, id).await
}

/// Accept the current state of one file as what you read.
#[tauri::command]
pub async fn resnapshot_doc_file(
    state: State<'_, AppState>,
    id: i64,
    path: String,
) -> AppResult<Reading> {
    let source = std::fs::read_to_string(Path::new(&path))?;
    let sha = sha256(&source);
    files_db::resnapshot(&state.pool, id, &path, &source, &sha).await?;
    // The origin's snapshot lives in two places, because `docs.source` is what
    // the library and the chooser read. Keep them in step.
    let doc = docs_db::get(&state.pool, id).await?;
    if doc.path == path {
        docs_db::update_source(&state.pool, id, &source, &sha, &doc.markdown).await?;
    }
    build_reading(&state, id).await
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
    // Both places the origin's snapshot lives, or it would still read as stale
    // in the file strip after a reconcile.
    files_db::resnapshot(&state.pool, id, &doc.path, &source, &sha).await?;
    docs_db::update_source(&state.pool, id, &source, &sha, &merged).await
}
