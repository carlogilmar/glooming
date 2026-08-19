//! Opening a folder, and listing what is in it.
//!
//! A project is just a path you search within. There is no index on disk and no
//! scan at startup: the walk runs when you open the picker and is fast enough
//! that caching it would be inventing a staleness problem to solve.

use crate::commands::AppState;
use crate::db::models::{Project, ProjectFile};
use crate::db::projects as db;
use crate::error::{AppError, AppResult};
use ignore::WalkBuilder;
use std::path::Path;
use tauri::State;

/// Directories that are never worth walking, on top of whatever `.gitignore`
/// already excludes — `_build` and `deps` in particular are usually ignored
/// anyway, but a fresh checkout may not have them ignored yet.
const SKIP: [&str; 6] = ["_build", "deps", "node_modules", ".git", ".elixir_ls", "cover"];

fn is_readable(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ex") | Some("exs")
    )
}

#[tauri::command]
pub async fn open_project(state: State<'_, AppState>, path: String) -> AppResult<Project> {
    let path = crate::commands::files::normalize_path(&path);
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(AppError::BadInput(format!("{path} is not a folder")));
    }
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    db::touch(&state.pool, &path, &name).await
}

#[tauri::command]
pub async fn recent_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    db::recent(&state.pool, 8).await
}

#[tauri::command]
pub async fn forget_project(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    db::forget(&state.pool, id).await
}

/// Every Elixir file under the project, as absolute path plus the relative one
/// you actually read and search.
#[tauri::command]
pub async fn project_files(path: String) -> AppResult<Vec<ProjectFile>> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(AppError::NotFound(format!("no folder at {path}")));
    }

    let mut out = Vec::new();
    let walk = WalkBuilder::new(root)
        // Honour .gitignore, which is what keeps build output out without a
        // hand-maintained list.
        .standard_filters(true)
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|n| SKIP.contains(&n))
                .unwrap_or(false)
        })
        .build();

    for entry in walk.flatten() {
        let p = entry.path();
        if !p.is_file() || !is_readable(p) {
            continue;
        }
        let rel = p.strip_prefix(root).unwrap_or(p).to_string_lossy().to_string();
        out.push(ProjectFile {
            name: p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: p.to_string_lossy().to_string(),
            rel,
        });
    }

    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_elixir_files_are_offered() {
        assert!(is_readable(Path::new("/a/accounts.ex")));
        assert!(is_readable(Path::new("/a/seeds.exs")));
        // lgtm can open anything, but the picker lists what it actually reads.
        assert!(!is_readable(Path::new("/a/README.md")));
        assert!(!is_readable(Path::new("/a/mix.lock")));
        assert!(!is_readable(Path::new("/a/logo.png")));
        assert!(!is_readable(Path::new("/a/Makefile")));
    }

    #[tokio::test]
    async fn walks_a_real_tree_and_skips_build_output() {
        let root = std::env::temp_dir().join(format!("lgtm-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["lib/my_app", "_build/dev", "deps/ecto", "test"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("lib/my_app/accounts.ex"), "defmodule A do\nend\n").unwrap();
        std::fs::write(root.join("test/accounts_test.exs"), "defmodule T do\nend\n").unwrap();
        std::fs::write(root.join("_build/dev/generated.ex"), "x").unwrap();
        std::fs::write(root.join("deps/ecto/ecto.ex"), "x").unwrap();
        std::fs::write(root.join("README.md"), "hi").unwrap();

        let files = project_files(root.to_string_lossy().to_string()).await.unwrap();
        let rels: Vec<&str> = files.iter().map(|f| f.rel.as_str()).collect();

        assert!(rels.contains(&"lib/my_app/accounts.ex"), "{rels:?}");
        assert!(rels.contains(&"test/accounts_test.exs"), "{rels:?}");
        // Build output and dependencies are somebody else's code.
        assert!(!rels.iter().any(|r| r.starts_with("_build")), "{rels:?}");
        assert!(!rels.iter().any(|r| r.starts_with("deps")), "{rels:?}");
        // Non-Elixir files are not offered.
        assert!(!rels.contains(&"README.md"), "{rels:?}");
        // Relative paths, since that is what you search on.
        assert!(files.iter().all(|f| !f.rel.starts_with('/')), "{rels:?}");

        std::fs::remove_dir_all(&root).ok();
    }
}
