//! Doc CRUD plus seeding. Thin wrappers — the logic lives in `db::docs`,
//! `seed`.

use crate::commands::files::{normalize_path, sha256};
use crate::commands::AppState;
use crate::db::models::{Doc, DocSummary, Reading, ReadingFile};
use crate::db::{doc_files as files_db, docs as docs_db};
use crate::error::AppResult;
use crate::parse::Outline;
use crate::seed;
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
/// One file of a gloom, as it was when you opened it.
///
/// **The snapshot is the file.** A gloom is a reading of a particular version, so
/// the pane shows what `doc_files.source` holds and the outline is parsed from
/// that — not from whatever is on disk now. That is what makes a line number in
/// your prose mean the same thing next month as it did today.
///
/// Disk is still consulted for exactly one thing: whether it has *moved on*. That
/// is worth saying — the code you are reading is no longer the code that runs —
/// but it is a fact, not an offer. There is nothing to reconcile: to read the new
/// version you start a new gloom, which is one gesture and leaves this reading
/// intact.
fn read_one(f: &crate::db::models::DocFile, origin: &str) -> ReadingFile {
    let p = Path::new(&f.path);
    let disk = std::fs::read_to_string(p).ok();
    let missing = disk.is_none();
    let source = f.source.clone();
    let source_sha = f.source_sha.clone();
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
        stale: match &disk {
            Some(now) => sha256(now) != f.source_sha,
            None => false,
        },
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

#[tauri::command]
pub async fn open_reading(state: State<'_, AppState>, id: i64) -> AppResult<Reading> {
    build_reading(&state, id).await
}

/// Add a file to a reading.
///
/// **Nothing is seeded.** The note is yours from the first file onward; what an
/// added file contributes is source to read and functions to reference, which
/// is exactly what a review needs and no more.
///
/// **And it must come from the branch the gloom was read on.** A gloom is a
/// reading of one version of a change; a file snapshotted from a different branch
/// is a different version of the world sitting silently in the same note, with
/// line numbers your prose will describe as if they were the same. Nothing later
/// can untangle that, so it is refused at the door.
#[tauri::command]
pub async fn add_doc_file(state: State<'_, AppState>, id: i64, path: String) -> AppResult<Reading> {
    let path = normalize_path(&path);
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(crate::error::AppError::NotFound(format!(
            "no file at {path}"
        )));
    }

    let doc = docs_db::get(&state.pool, id).await?;
    if let Some(problem) = branch_mismatch(&doc, p) {
        return Err(crate::error::AppError::BadInput(problem));
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

/// Why this file may not join this gloom, if it may not.
///
/// Deliberately narrow — it refuses only when it is **sure**, because a false
/// refusal blocks work and a false pass costs one confusing note:
///
/// - both the gloom and the candidate must be in a repository, and the **same**
///   one: a file from another checkout has branch names that mean nothing here;
/// - both branches must be readable. A detached HEAD, a fresh repo with no
///   commits, a file outside git — all pass, because "I cannot tell" is not "no".
fn branch_mismatch(doc: &Doc, candidate: &Path) -> Option<String> {
    let want = doc.branch.as_deref()?;
    let here = crate::git::current_branch(candidate)?;
    if here == want {
        return None;
    }
    let origin_root = crate::git::repo_root(Path::new(&doc.path))?;
    let candidate_root = crate::git::repo_root(candidate)?;
    if origin_root != candidate_root {
        return None;
    }
    Some(format!(
        "This gloom was read on {want}, and you are on {here}. \
         Adding a file now would put two versions of the same code in one reading. \
         Check out {want} to keep going, or start a new gloom for {here}."
    ))
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



#[cfg(test)]
mod branch_tests {
    use super::*;
    use crate::db::models::Doc;

    fn doc(path: &str, branch: Option<&str>) -> Doc {
        Doc {
            id: 1,
            path: path.into(),
            filename: "accounts.ex".into(),
            lang: "elixir".into(),
            title: "Accounts".into(),
            branch: branch.map(str::to_string),
            label: None,
            markdown: String::new(),
            source: String::new(),
            source_sha: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    /// A gloom with no branch — a file outside git, a fresh repo — never refuses.
    /// "I cannot tell" is not "no": a false refusal blocks work, and a false pass
    /// costs one confusing note.
    #[test]
    fn a_gloom_without_a_branch_accepts_anything() {
        assert!(branch_mismatch(&doc("/tmp/a.ex", None), Path::new("/tmp/b.ex")).is_none());
    }

    /// And a candidate outside git passes for the same reason: `current_branch`
    /// returns `None`, and the guard only fires on two known, differing names.
    #[test]
    fn a_file_outside_git_is_not_refused() {
        let d = doc("/tmp/a.ex", Some("main"));
        assert!(branch_mismatch(&d, Path::new("/tmp/definitely/not/a/repo/b.ex")).is_none());
    }
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
        let md = "# MyApp.Accounts\n\n> Reads and writes.\n\n```lgtm:stats\nlines: 42\n```\n";
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
