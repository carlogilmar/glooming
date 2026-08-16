//! Reading, parsing and blaming a file on disk. No writes ever happen here —
//! lgtm reads code, it does not edit it.

use crate::commands::AppState;
use crate::db::models::{BlameLine, OpenedFile};
use crate::db::{docs as docs_db, models::DocSummary};
use crate::error::{AppError, AppResult};
use crate::git;
use crate::parse::{self, Outline};
use sha2::{Digest, Sha256};
use std::path::Path;
use tauri::State;

pub fn sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn filename_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Read a file, parse it if we know the language, and report any docs already
/// written about this path. One round trip — the UI renders from one payload.
#[tauri::command]
pub async fn open_file(state: State<'_, AppState>, path: String) -> AppResult<OpenedFile> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(AppError::NotFound(format!("no file at {path}")));
    }
    let source = std::fs::read_to_string(p)?;
    let source_sha = sha256(&source);
    let lang = parse::lang_for_path(&path).map(str::to_string);

    // A parse failure must not stop you reading the file — you just get no
    // outline to seed from.
    let outline = match &lang {
        Some(l) => parse::parse(&source, l).ok(),
        None => None,
    };

    let existing: Vec<DocSummary> = docs_db::for_path(&state.pool, &path).await?;

    Ok(OpenedFile {
        filename: filename_of(&path),
        branch: git::current_branch(p),
        has_git: git::repo_root(p).is_some(),
        path,
        source,
        source_sha,
        lang,
        outline,
        existing,
    })
}

/// Re-read and re-parse a file already on screen (the Re-parse button, and the
/// staleness check when an existing doc is opened).
#[tauri::command]
pub async fn reparse(path: String) -> AppResult<OpenedFile> {
    let p = Path::new(&path);
    let source = std::fs::read_to_string(p)?;
    let source_sha = sha256(&source);
    let lang = parse::lang_for_path(&path).map(str::to_string);
    let outline: Option<Outline> = match &lang {
        Some(l) => parse::parse(&source, l).ok(),
        None => None,
    };

    Ok(OpenedFile {
        filename: filename_of(&path),
        branch: git::current_branch(p),
        has_git: git::repo_root(p).is_some(),
        path,
        source,
        source_sha,
        lang,
        outline,
        existing: Vec::new(),
    })
}

/// Lazy: only invoked when the Blame button is pressed.
#[tauri::command]
pub async fn blame_file(path: String) -> AppResult<Vec<BlameLine>> {
    git::blame(Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_is_stable_and_content_sensitive() {
        assert_eq!(sha256("def x"), sha256("def x"));
        assert_ne!(sha256("def x"), sha256("def y"));
    }

    #[test]
    fn filename_is_the_last_component() {
        assert_eq!(filename_of("/a/b/accounts.ex"), "accounts.ex");
        assert_eq!(filename_of("accounts.ex"), "accounts.ex");
    }
}
