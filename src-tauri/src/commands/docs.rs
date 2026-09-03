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

// ---- importing a gloom ----------------------------------------------------

/// What the disk says about one file the header listed.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    pub path: String,
    pub line: u32,
    pub found: bool,
}

/// Whether the gloom's branch is the one you are standing on.
///
/// `Unchecked` is a first-class answer, not a failure. The check is narrow for
/// the same reason `branch_mismatch` is: it speaks only when it is **sure**.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum BranchCheck {
    /// Not a repository, no readable branch, or the file does not say.
    Unchecked { why: String },
    Same { branch: String },
    Differs { wants: String, here: String },
}

/// Everything the panel needs, in one call: nothing is asked for twice and the
/// frontend never has to sequence a read against a check.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub name: String,
    /// The project as the file wrote it — shown even when it does not resolve.
    pub project: String,
    /// The directory actually checked: `project`, or the one you chose instead.
    pub root: String,
    pub root_ok: bool,
    /// True when `root` came from the picker rather than from the file.
    pub root_chosen: bool,
    pub branch: BranchCheck,
    pub files: Vec<PreviewFile>,
    pub note_bytes: usize,
    pub problems: Vec<crate::import::Problem>,
    /// Every check passed. **Import is offered only when this is true** — a
    /// missing file, a missing directory and a different branch all block it.
    pub ready: bool,
}

/// A gloom file with the parts we already know filled in.
///
/// The branch is read here rather than passed in, so the frontend never has to
/// sequence two calls to produce one piece of text — the same habit `seed_doc`
/// follows for git history.
#[tauri::command]
pub fn gloom_template(project: Option<String>) -> String {
    let project = project.map(|p| normalize_path(&p));
    let branch = project
        .as_deref()
        .and_then(|p| crate::git::current_branch(Path::new(p)));
    crate::import::template(project.as_deref(), branch.as_deref())
}

/// Read a gloom file and say what is wrong with it, if anything.
///
/// Loading and validating are one step because they are one thought: you chose a
/// file, and what you want to know is whether it will work. A separate
/// "validate" button would be a control whose only purpose is to ask a question
/// the app can already answer.
#[tauri::command]
pub fn preview_import(path: String, root: Option<String>) -> AppResult<ImportPreview> {
    let file = normalize_path(&path);
    let text = std::fs::read_to_string(&file)?;
    Ok(inspect(&text, root.as_deref()))
}

/// The whole check, over text rather than a path, so it is testable without a
/// gloom file on disk.
pub fn inspect(text: &str, chosen: Option<&str>) -> ImportPreview {
    let parsed = crate::import::parse(text);

    let root_chosen = chosen.is_some();
    let root_raw = chosen.unwrap_or(&parsed.project);
    let root = normalize_path(root_raw);
    let root_path = Path::new(&root);
    let root_ok = !parsed.project.is_empty() && root_path.is_dir();

    let files: Vec<PreviewFile> = parsed
        .files
        .iter()
        .map(|f| PreviewFile {
            found: root_ok && root_path.join(&f.path).is_file(),
            path: f.path.clone(),
            line: f.line,
        })
        .collect();

    let branch = branch_check(&parsed, root_path, root_ok);

    // Strict, and in that order: the header has to be readable before the disk
    // is worth consulting, and the branch only means anything once the files
    // are actually there.
    let ready = parsed.ok()
        && root_ok
        && !files.is_empty()
        && files.iter().all(|f| f.found)
        && !matches!(branch, BranchCheck::Differs { .. });

    ImportPreview {
        name: parsed.name.clone(),
        project: parsed.project.clone(),
        root,
        root_ok,
        root_chosen,
        branch,
        files,
        note_bytes: parsed.note.len(),
        problems: parsed.problems,
        ready,
    }
}

/// Narrow by design — `Unchecked` wherever the answer would be a guess.
///
/// A file with no `branch:` imports without a word about git, and so does a
/// directory that is not a repository, a detached HEAD, and a repo with no
/// commits. "I cannot tell" is not "no", which is the policy `branch_mismatch`
/// already follows for adding a file to an existing gloom.
fn branch_check(parsed: &crate::import::Parsed, root: &Path, root_ok: bool) -> BranchCheck {
    let Some(wants) = parsed.branch.as_deref() else {
        return BranchCheck::Unchecked {
            why: "the file does not say which branch it was read on".into(),
        };
    };
    if !root_ok {
        return BranchCheck::Unchecked {
            why: "the project directory is not here".into(),
        };
    }
    if crate::git::repo_root(root).is_none() {
        return BranchCheck::Unchecked {
            why: "the project is not a git repository".into(),
        };
    }
    let Some(here) = crate::git::current_branch(root) else {
        return BranchCheck::Unchecked {
            why: "no branch is checked out here — a detached HEAD, or no commits yet".into(),
        };
    };
    if here == wants {
        BranchCheck::Same {
            branch: here,
        }
    } else {
        BranchCheck::Differs {
            wants: wants.to_string(),
            here,
        }
    }
}

/// Create a gloom from a file, or refuse and say why.
///
/// **Re-validated from scratch**, never from the preview the panel is showing.
/// The branch in particular has to be read at the gesture rather than when the
/// panel opened — you can check something out in a terminal while a dialog is on
/// screen, and a check only as fresh as the last render is not fresh enough for
/// the one action it guards.
///
/// The first file is the **origin**: `docs.path`, what the library groups by,
/// and the one file that cannot be removed. The gloom's branch is the one *you*
/// are on, because that is where its snapshots came from — the header's `branch:`
/// was provenance, and it is not kept.
#[tauri::command]
pub async fn import_gloom(
    state: State<'_, AppState>,
    path: String,
    root: Option<String>,
) -> AppResult<Reading> {
    let file = normalize_path(&path);
    let text = std::fs::read_to_string(&file)?;
    let id = create_from_file(&state.pool, &text, root.as_deref()).await?;
    build_reading(&state, id).await
}

/// Create the gloom, or refuse and say why. Returns the new doc's id.
///
/// Takes a pool rather than `State` so the whole chain — parse, disk, git, rows
/// — is testable without a Tauri app around it. The command above is the thin
/// part, which is where the thin part belongs.
///
/// **Re-validated from scratch**, never from the preview the panel is showing.
/// The branch in particular is read *here*, at the gesture, rather than when the
/// panel opened: you can check something out in a terminal while a dialog is on
/// screen, and a check only as fresh as the last render is not fresh enough for
/// the one action it guards.
pub async fn create_from_file(
    pool: &sqlx::SqlitePool,
    text: &str,
    root: Option<&str>,
) -> AppResult<i64> {
    let preview = inspect(text, root);
    if !preview.ready {
        return Err(crate::error::AppError::BadInput(refusal(&preview)));
    }

    let parsed = crate::import::parse(text);
    let root_path = Path::new(&preview.root);
    let branch = crate::git::current_branch(root_path);

    // The first file is the ORIGIN: `docs.path`, what the library groups by, and
    // the one file that cannot be removed. The gloom's branch is the one *you*
    // are on, because that is where these snapshots came from — the header's
    // `branch:` was provenance, and it is not kept.
    let mut id: Option<i64> = None;
    for f in &parsed.files {
        let full = root_path.join(&f.path);
        let full_str = full.to_string_lossy().to_string();
        let source = std::fs::read_to_string(&full)?;
        let sha = sha256(&source);
        let filename = full
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| f.path.clone());
        let lang = crate::parse::lang_for_path(&full_str).unwrap_or("text");

        let doc_id = match id {
            Some(existing) => existing,
            None => {
                let doc = docs_db::create(
                    pool,
                    &full_str,
                    &filename,
                    lang,
                    &parsed.name,
                    branch.as_deref(),
                    // The note lands verbatim. Nothing is seeded over it: an
                    // imported gloom arrives already written.
                    &parsed.note,
                    &source,
                    &sha,
                )
                .await?;
                id = Some(doc.id);
                doc.id
            }
        };
        files_db::add(pool, doc_id, &full_str, &filename, lang, &source, &sha).await?;
    }

    id.ok_or_else(|| crate::error::AppError::BadInput("this file lists no files".into()))
}

/// One sentence saying why, for the case the UI should have prevented.
fn refusal(p: &ImportPreview) -> String {
    if let Some(first) = p.problems.first() {
        return match first.line {
            Some(n) => format!("line {n}: {}", first.message),
            None => first.message.clone(),
        };
    }
    if !p.root_ok {
        return format!("{} is not a directory on this machine", p.root);
    }
    let missing: Vec<&str> = p
        .files
        .iter()
        .filter(|f| !f.found)
        .map(|f| f.path.as_str())
        .collect();
    if !missing.is_empty() {
        return format!("not found under {}: {}", p.root, missing.join(", "));
    }
    if let BranchCheck::Differs { wants, here } = &p.branch {
        return format!(
            "This gloom was read on {wants}, and you are on {here}. \
             Check out {wants} to import it as it was written."
        );
    }
    "this file cannot be imported".into()
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

/// A throwaway project on disk, shared by the preview tests and the end-to-end
/// ones. Real directories and real files, because the whole point of these
/// checks is the disk and a mock would test nothing.
#[cfg(test)]
pub mod import_tests_support {
    pub struct Project(pub std::path::PathBuf);

    pub const BOTH: &str = "  - lib/my_app/accounts.ex\n  - lib/my_app/billing.ex\n";

    pub fn project(tag: &str) -> Project {
        Project::new(tag)
    }

    impl Project {
        pub fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("lgtm-import-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(dir.join("lib/my_app")).expect("dirs");
            std::fs::write(dir.join("lib/my_app/accounts.ex"), "defmodule A do\nend\n").expect("a");
            std::fs::write(dir.join("lib/my_app/billing.ex"), "defmodule B do\nend\n").expect("b");
            Project(dir)
        }
        pub fn file(&self, files: &str, extra: &str) -> String {
            format!(
                "---\nproject: {}\nname: A gloom\n{extra}files:\n{files}---\n\nThe note.\n",
                self.0.display()
            )
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
}

#[cfg(test)]
mod import_tests {
    use super::import_tests_support::{Project, BOTH};
    use super::{inspect, BranchCheck};

    #[test]
    fn a_file_whose_every_part_resolves_is_ready() {
        let p = Project::new("ready");
        let v = inspect(&p.file(BOTH, ""), None);
        assert!(v.problems.is_empty(), "{:?}", v.problems);
        assert!(v.root_ok);
        assert!(v.files.iter().all(|f| f.found));
        assert!(v.ready, "{v:?}");
        assert_eq!(v.name, "A gloom");
        assert_eq!(v.files[0].path, "lib/my_app/accounts.ex", "the origin is first");
    }

    #[test]
    fn a_missing_file_is_named_and_blocks() {
        let p = Project::new("missing");
        let listed = "  - lib/my_app/accounts.ex\n  - lib/my_app/legacy.ex\n";
        let v = inspect(&p.file(listed, ""), None);

        assert!(v.root_ok, "the directory is fine — only the file is not");
        assert!(v.problems.is_empty(), "the file itself is well-formed");
        let missing: Vec<&str> = v.files.iter().filter(|f| !f.found).map(|f| f.path.as_str()).collect();
        assert_eq!(missing, vec!["lib/my_app/legacy.ex"]);
        assert!(!v.ready);
    }

    #[test]
    fn a_directory_that_is_not_here_blocks_and_reports_no_files() {
        let v = inspect(
            "---\nproject: /no/such/place\nname: n\nfiles:\n  - a.ex\n---\nnote",
            None,
        );
        assert!(!v.root_ok);
        assert!(!v.files[0].found, "nothing can be found under a root that is not there");
        assert!(!v.ready);
    }

    /// The colleague case: their checkout is somewhere else, and every check
    /// re-runs against the root they pick rather than the one the file names.
    #[test]
    fn choosing_a_root_re_resolves_every_file_against_it() {
        let p = Project::new("chosen");
        let text = format!(
            "---\nproject: /somewhere/else\nname: n\nfiles:\n{BOTH}---\nnote"
        );

        let before = inspect(&text, None);
        assert!(!before.root_ok && !before.ready);

        let after = inspect(&text, Some(&p.0.to_string_lossy()));
        assert!(after.root_ok && after.ready, "{after:?}");
        assert!(after.root_chosen, "the panel can say the root was chosen here");
        assert_eq!(after.project, "/somewhere/else", "the file's own value is still shown");
    }

    /// Narrow: a temp directory is not a repository, so there is nothing to
    /// compare and the check says so instead of guessing.
    #[test]
    fn a_project_outside_git_is_not_refused() {
        let p = Project::new("nogit");
        let v = inspect(&p.file(BOTH, "branch: main\n"), None);
        assert!(matches!(v.branch, BranchCheck::Unchecked { .. }), "{:?}", v.branch);
        assert!(v.ready, "an unreadable branch must never block");
    }

    #[test]
    fn a_file_without_a_branch_key_is_never_asked_about_git() {
        let p = Project::new("nobranch");
        let v = inspect(&p.file(BOTH, ""), None);
        match &v.branch {
            BranchCheck::Unchecked { why } => assert!(why.contains("does not say"), "{why}"),
            other => panic!("expected Unchecked, got {other:?}"),
        }
        assert!(v.ready);
    }

    /// A real repository, so the branch arms are exercised rather than assumed.
    /// `git init` is cheap and this is the one check that cannot be faked.
    fn git_project(tag: &str) -> Option<Project> {
        let p = Project::new(tag);
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&p.0)
                .output()
                .ok()
                .filter(|o| o.status.success())
        };
        run(&["init", "-b", "main"])?;
        run(&["config", "user.email", "t@example.com"])?;
        run(&["config", "user.name", "T"])?;
        run(&["add", "."])?;
        run(&["commit", "-m", "first"])?;
        Some(p)
    }

    #[test]
    fn the_same_branch_passes_and_says_so() {
        let Some(p) = git_project("same") else { return };
        let v = inspect(&p.file(BOTH, "branch: main\n"), None);
        match &v.branch {
            BranchCheck::Same { branch } => assert_eq!(branch, "main"),
            other => panic!("expected Same, got {other:?}"),
        }
        assert!(v.ready);
    }

    /// Strict: every file resolved, and it still does not import. A gloom is a
    /// reading of one version, and importing it onto another branch would put a
    /// note describing `main` over code from somewhere else.
    #[test]
    fn a_different_branch_blocks_even_when_every_file_is_there() {
        let Some(p) = git_project("differs") else { return };
        let v = inspect(&p.file(BOTH, "branch: release/2.4\n"), None);

        assert!(v.files.iter().all(|f| f.found), "the files are all present");
        assert!(v.problems.is_empty(), "the file itself is fine");
        match &v.branch {
            BranchCheck::Differs { wants, here } => {
                assert_eq!(wants, "release/2.4");
                assert_eq!(here, "main");
            }
            other => panic!("expected Differs, got {other:?}"),
        }
        assert!(!v.ready, "a branch mismatch blocks the import");
    }

    #[test]
    fn a_malformed_header_blocks_before_the_disk_is_consulted() {
        let p = Project::new("malformed");
        let text = format!(
            "---\nprojekt: {}\nname: n\nfiles:\n{BOTH}---\nnote",
            p.0.display()
        );
        let v = inspect(&text, None);
        assert!(!v.ready);
        assert!(v.problems.iter().any(|x| x.message.contains("unknown key `projekt`")));
        assert!(v.problems.iter().any(|x| x.message.contains("`project` is missing")));
    }

    #[test]
    fn the_note_is_measured_not_the_whole_file() {
        let p = Project::new("bytes");
        let v = inspect(&p.file(BOTH, ""), None);
        assert_eq!(v.note_bytes, "The note.\n".len());
    }
}

#[cfg(test)]
mod import_end_to_end {
    use super::{create_from_file, import_tests_support::*};
    use crate::db::{doc_files as files_db, docs as docs_db, test_pool};

    #[tokio::test]
    async fn a_gloom_file_becomes_a_gloom() {
        let p = project("e2e");
        let pool = test_pool().await;

        let text = p.file(BOTH, "");
        let id = create_from_file(&pool, &text, None).await.expect("imports");

        let doc = docs_db::get(&pool, id).await.expect("the doc");
        assert_eq!(doc.title, "A gloom", "the name comes from the header");
        assert_eq!(doc.markdown, "The note.\n", "the note lands verbatim");
        assert!(
            doc.path.ends_with("lib/my_app/accounts.ex"),
            "the first file is the origin: {}",
            doc.path
        );

        let files = files_db::list(&pool, id).await.expect("its files");
        assert_eq!(files.len(), 2, "every listed file joined");
        assert!(files[0].path.ends_with("accounts.ex"), "in the order written");
        assert!(files[1].path.ends_with("billing.ex"));
        assert_eq!(files[0].source, "defmodule A do\nend\n", "snapshotted from disk");
    }

    /// The strictness, at the door rather than only in the panel. A UI that
    /// disables the button is a convenience; this is the rule.
    #[tokio::test]
    async fn a_missing_file_creates_nothing_at_all() {
        let p = project("e2e-missing");
        let pool = test_pool().await;

        let text = p.file("  - lib/my_app/accounts.ex\n  - lib/my_app/gone.ex\n", "");
        let err = create_from_file(&pool, &text, None)
            .await
            .expect_err("refuses");
        assert!(err.to_string().contains("gone.ex"), "names it: {err}");

        // Not "created the ones it could find" — nothing.
        assert!(docs_db::list(&pool, None, 50).await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn a_malformed_header_creates_nothing_and_names_the_line() {
        let p = project("e2e-bad");
        let pool = test_pool().await;

        let text = format!(
            "---\nprojekt: {}\nname: n\nfiles:\n{BOTH}---\nnote",
            p.0.display()
        );
        let err = create_from_file(&pool, &text, None).await.expect_err("refuses");
        assert!(err.to_string().starts_with("invalid input: line 2:"), "{err}");
        assert!(docs_db::list(&pool, None, 50).await.expect("list").is_empty());
    }

    /// Importing twice makes two glooms — no sync, no re-import, no "update
    /// from file". The same call as opening a changed file as a new gloom.
    #[tokio::test]
    async fn importing_twice_makes_two_glooms() {
        let p = project("e2e-twice");
        let pool = test_pool().await;
        let text = p.file(BOTH, "");

        let a = create_from_file(&pool, &text, None).await.expect("first");
        let b = create_from_file(&pool, &text, None).await.expect("second");
        assert_ne!(a, b);
        assert_eq!(docs_db::list(&pool, None, 50).await.expect("list").len(), 2);
    }

    #[tokio::test]
    async fn a_chosen_root_is_what_the_files_are_read_from() {
        let p = project("e2e-root");
        let pool = test_pool().await;

        let text = format!("---\nproject: /somewhere/else\nname: n\nfiles:\n{BOTH}---\nnote");
        assert!(create_from_file(&pool, &text, None).await.is_err());

        let id = create_from_file(&pool, &text, Some(&p.0.to_string_lossy()))
            .await
            .expect("imports against the chosen root");
        let doc = docs_db::get(&pool, id).await.expect("doc");
        assert!(doc.path.starts_with(&p.0.to_string_lossy().to_string()), "{}", doc.path);
    }
}
