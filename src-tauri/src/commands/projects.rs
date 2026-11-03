//! Opening a folder, and listing what is in it.
//!
//! A project is just a path you search within. There is no index on disk and no
//! scan at startup: the walk runs when you open the picker and is fast enough
//! that caching it would be inventing a staleness problem to solve.

use crate::commands::AppState;
use crate::db::models::{Project, ProjectFile};
use crate::db::projects as db;
use crate::error::{AppError, AppResult};
use crate::index::IndexedModule;
use ignore::WalkBuilder;
use std::path::Path;
use tauri::State;

/// Never worth showing, on top of whatever `.gitignore` already excludes —
/// `_build` and `deps` in particular are usually ignored anyway, but a fresh
/// checkout may not have them ignored yet. Editor and OS litter too: it is in
/// the folder, but nobody browsing a codebase means it.
const SKIP: [&str; 11] = [
    "_build", "deps", "node_modules", ".git", ".elixir_ls", ".lexical", "cover",
    ".DS_Store", ".hex", ".mix", "erl_crash.dump",
];

/// The files the module index scans: the ones with Elixir in them. Every other
/// file is listed and readable as text; only these can define a module.
fn is_elixir(path: &Path) -> bool {
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

/// Every file under a folder, as an editor's file tree would show it, sorted by
/// relative path.
///
/// **Every file, not only Elixir.** Reading a codebase means `mix.exs`, the
/// config, the migrations, the assets and the README as much as the modules; a
/// tree that shows only what lgtm can parse is a partial map that looks complete.
/// Anything without a parser opens as plain text, read-only.
///
/// Dotfiles are shown — `.formatter.exs` and `.gitignore` are part of a project —
/// while `.gitignore` itself, and `SKIP`, keep out what is not.
fn walk(root: &Path) -> Vec<ProjectFile> {
    let mut out = Vec::new();
    let walk = WalkBuilder::new(root)
        // Honour .gitignore, which is what keeps build output out without a
        // hand-maintained list — and do so even in a folder that is not a git
        // checkout, where its .gitignore still says what the author meant.
        .standard_filters(true)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|n| SKIP.contains(&n))
                .unwrap_or(false)
        })
        .build();

    for entry in walk.flatten() {
        let p = entry.path();
        if !p.is_file() {
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
    out
}

/// Every file under the project, as absolute path plus the relative one you
/// actually read and search — the sidebar's tree and `⌘T` both read this.
#[tauri::command]
pub async fn project_files(path: String) -> AppResult<Vec<ProjectFile>> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(AppError::NotFound(format!("no folder at {path}")));
    }
    Ok(walk(root))
}

/// Every module in the project and the file it lives in — see `index.rs`.
///
/// What lets a reference name any module in the codebase, and what the sidebar's
/// namespace tree is drawn from. Rebuilt on every call: derived, never stored.
#[tauri::command]
pub async fn project_index(path: String) -> AppResult<Vec<IndexedModule>> {
    let root = Path::new(&path);
    if !root.is_dir() {
        return Err(AppError::NotFound(format!("no folder at {path}")));
    }
    Ok(index_of(root))
}

pub fn index_of(root: &Path) -> Vec<IndexedModule> {
    let mut out = Vec::new();
    for f in walk(root).into_iter().filter(|f| is_elixir(Path::new(&f.path))) {
        // An unreadable file is one fewer entry, never a failed index.
        let Ok(src) = std::fs::read_to_string(&f.path) else { continue };
        let test = crate::index::is_test(&f.rel);
        for module in crate::index::module_names(&src) {
            out.push(IndexedModule { module, path: f.path.clone(), rel: f.rel.clone(), test });
        }
    }
    out.sort_by(|a, b| a.module.cmp(&b.module));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_elixir_files_are_indexed() {
        assert!(is_elixir(Path::new("/a/accounts.ex")));
        assert!(is_elixir(Path::new("/a/seeds.exs")));
        assert!(!is_elixir(Path::new("/a/README.md")));
        assert!(!is_elixir(Path::new("/a/mix.lock")));
        assert!(!is_elixir(Path::new("/a/Makefile")));
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
        std::fs::write(root.join(".formatter.exs"), "[]").unwrap();
        std::fs::write(root.join(".DS_Store"), "x").unwrap();
        std::fs::write(root.join(".gitignore"), "/secret/\n").unwrap();
        std::fs::create_dir_all(root.join("secret")).unwrap();
        std::fs::write(root.join("secret/key.txt"), "x").unwrap();

        let files = project_files(root.to_string_lossy().to_string()).await.unwrap();
        let rels: Vec<&str> = files.iter().map(|f| f.rel.as_str()).collect();

        assert!(rels.contains(&"lib/my_app/accounts.ex"), "{rels:?}");
        assert!(rels.contains(&"test/accounts_test.exs"), "{rels:?}");
        // Build output and dependencies are somebody else's code.
        assert!(!rels.iter().any(|r| r.starts_with("_build")), "{rels:?}");
        assert!(!rels.iter().any(|r| r.starts_with("deps")), "{rels:?}");
        // Every file is in the tree, not only what lgtm can parse.
        assert!(rels.contains(&"README.md"), "{rels:?}");
        // Dotfiles are part of a project; OS litter is not.
        assert!(rels.contains(&".formatter.exs"), "{rels:?}");
        assert!(!rels.contains(&".DS_Store"), "{rels:?}");
        // .gitignore is honoured even outside a git checkout.
        assert!(!rels.iter().any(|r| r.starts_with("secret")), "{rels:?}");
        // Relative paths, since that is what you search on.
        assert!(files.iter().all(|f| !f.rel.starts_with('/')), "{rels:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_index_maps_every_module_to_its_file() {
        let root = std::env::temp_dir().join(format!("lgtm-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["lib/weather", "lib/ai", "test", "config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("lib/weather/pipeline.ex"), "defmodule Weather.Pipeline do\nend\n").unwrap();
        std::fs::write(root.join("lib/ai/pipeline.ex"), "defmodule Ai.Pipeline do\n  defmodule Chunk do\n  end\nend\n").unwrap();
        std::fs::write(root.join("test/pipeline_test.exs"), "defmodule Weather.PipelineTest do\nend\n").unwrap();
        std::fs::write(root.join("config/config.exs"), "import Config\n").unwrap();

        let idx = index_of(&root);
        let names: Vec<&str> = idx.iter().map(|m| m.module.as_str()).collect();
        assert_eq!(names, ["Ai.Pipeline", "Ai.Pipeline.Chunk", "Weather.Pipeline", "Weather.PipelineTest"]);

        // Two modules called Pipeline, each knowing its own file.
        let at = |m: &str| idx.iter().find(|e| e.module == m).unwrap();
        assert_eq!(at("Weather.Pipeline").rel, "lib/weather/pipeline.ex");
        assert_eq!(at("Ai.Pipeline.Chunk").rel, "lib/ai/pipeline.ex");
        assert!(at("Weather.PipelineTest").test);
        assert!(!at("Weather.Pipeline").test);

        std::fs::remove_dir_all(&root).ok();
    }
}
