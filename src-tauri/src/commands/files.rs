//! Reading, parsing and blaming a file on disk. No writes ever happen here —
//! lgtm reads code, it does not edit it.

use crate::db::models::{BlameLine, OpenedFile};
use crate::error::{AppError, AppResult};
use crate::git;
use crate::parse;
use std::path::Path;

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

/// Larger than this is a generated file or a data dump, not something to read
/// line by line — and highlighting it would stall the pane.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// A file's text, or a sentence saying why it has none.
///
/// The tree lists every file in the project, so a click can land on a PNG, a
/// `.beam` or a 40MB fixture. `read_to_string` reports those as `stream did not
/// contain valid UTF-8`, which is true and says nothing to someone who clicked a
/// logo. A NUL byte in the first 8KB is how git itself decides a file is binary.
fn read_text(p: &Path) -> AppResult<String> {
    let name = filename_of(&p.to_string_lossy());
    let size = std::fs::metadata(p)?.len();
    if size > MAX_BYTES {
        return Err(AppError::BadInput(format!(
            "{name} is {:.1} MB — too large to read here",
            size as f64 / 1_048_576.0
        )));
    }
    let bytes = std::fs::read(p)?;
    let binary = || AppError::BadInput(format!("{name} is a binary file — there is no text to show"));
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Err(binary());
    }
    String::from_utf8(bytes).map_err(|_| binary())
}

/// Read a file as it is on disk right now, and parse it if we know the language.
///
/// Always live: a gloom holds no copy of the code, so this is the only version
/// there is. One round trip — the UI renders from one payload.
#[tauri::command]
pub async fn open_file(path: String) -> AppResult<OpenedFile> {
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
    let source = read_text(p)?;
    let lang = parse::lang_for_path(&path).map(str::to_string);

    // A parse failure must not stop you reading the file — you just get no
    // outline to navigate by.
    let outline = match &lang {
        Some(l) => parse::parse(&source, l).ok(),
        None => None,
    };

    Ok(OpenedFile {
        filename: filename_of(&path),
        branch: git::current_branch(p),
        has_git: git::repo_root(p).is_some(),
        path,
        source,
        lang,
        outline,
    })
}

/// The branch a path is on, right now — the label in the app header.
///
/// Asked for rather than remembered: you check out another branch in a
/// terminal, and nothing in the app has any reason to notice. One read of
/// `.git/HEAD`. Nothing is pinned to it; it is a fact about where you are.
#[tauri::command]
pub fn branch_of(path: String) -> Option<String> {
    crate::git::current_branch(Path::new(&normalize_path(&path)))
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
    fn a_binary_file_says_so_rather_than_failing_on_utf8() {
        let dir = std::env::temp_dir().join(format!("lgtm-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("logo.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0, 0, 0, 13]).unwrap();
        let err = read_text(&png).expect_err("refuses").to_string();
        assert!(err.contains("logo.png is a binary file"), "got: {err}");

        let md = dir.join("README.md");
        std::fs::write(&md, "# hi\n").unwrap();
        assert_eq!(read_text(&md).unwrap(), "# hi\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn filename_is_the_last_component() {
        assert_eq!(filename_of("/a/b/accounts.ex"), "accounts.ex");
        assert_eq!(filename_of("accounts.ex"), "accounts.ex");
    }
}
