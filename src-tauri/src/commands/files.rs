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

/// Clean up a pasted path.
///
/// A path arriving by copy-paste is rarely clean: it may carry a trailing
/// newline from a terminal, `file://` from a browser or Finder, wrapping quotes
/// from a shell, backslash-escaped spaces from a drag-and-drop, or a leading
/// `~`. Normalizing here rather than in the frontend keeps the rules in one
/// testable place, and only Rust knows the real home directory.
pub fn normalize_path(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    if let Some(rest) = s.strip_prefix("file://") {
        s = rest.to_string();
    }

    // Wrapping quotes, but only when they match at both ends.
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            s = s[1..s.len() - 1].to_string();
        }
    }

    // `My\ Files` — how a shell quotes a space.
    s = s.replace("\\ ", " ");
    s = s.trim().to_string();

    if s == "~" || s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            let rest = s.strip_prefix('~').unwrap_or("").trim_start_matches('/');
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    s
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
    let path = normalize_path(&path);
    let p = Path::new(&path);
    if !p.is_file() {
        if p.is_dir() {
            return Err(AppError::BadInput(format!(
                "{path} is a directory — point at a file"
            )));
        }
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
    let path = normalize_path(&path);
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
    fn normalizes_the_ways_a_path_arrives_pasted() {
        // Whitespace and newlines from a terminal copy.
        assert_eq!(normalize_path("  /a/b.ex \n"), "/a/b.ex");
        // Finder / browser scheme.
        assert_eq!(normalize_path("file:///a/b.ex"), "/a/b.ex");
        // Shell quoting, both flavours.
        assert_eq!(normalize_path("\"/a/b.ex\""), "/a/b.ex");
        assert_eq!(normalize_path("'/a/b.ex'"), "/a/b.ex");
        // Drag-and-drop escaping.
        assert_eq!(normalize_path("/a/My\\ Files/b.ex"), "/a/My Files/b.ex");
        // A clean path is left exactly alone.
        assert_eq!(normalize_path("/a/b.ex"), "/a/b.ex");
    }

    #[test]
    fn expands_a_leading_tilde() {
        let home = dirs::home_dir().expect("home");
        let expected = home.join("code/a.ex").to_string_lossy().to_string();
        assert_eq!(normalize_path("~/code/a.ex"), expected);
        // A tilde anywhere else is a legitimate filename character.
        assert_eq!(normalize_path("/a/~b.ex"), "/a/~b.ex");
    }

    #[test]
    fn filename_is_the_last_component() {
        assert_eq!(filename_of("/a/b/accounts.ex"), "accounts.ex");
        assert_eq!(filename_of("accounts.ex"), "accounts.ex");
    }
}
