//! The only place git is touched, and it is touched lightly.
//!
//! * The branch label is a **plain file read** of `.git/HEAD` — no git binary.
//! * Blame shells out to `git blame`, read-only, and only when asked.
//!
//! Nothing here mutates a repository, and every function degrades to `None` /
//! empty outside one.

use crate::db::models::{BlameLine, FileHistory};
use crate::error::AppResult;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Walk up from a path to the nearest directory containing `.git`.
///
/// Starts *at* the path when it is a directory, and at its parent otherwise.
/// Every caller but one passes a file, for which this is unchanged — but the
/// import check asks about a project **directory**, and starting at the parent
/// silently skipped the repository it was standing in and answered "not a git
/// repository" for a repository.
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_dir() { path } else { path.parent()? };
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// The current branch, read straight out of `.git/HEAD`.
///
/// `ref: refs/heads/feature/x` → `feature/x`. A detached HEAD holds a raw sha,
/// which we shorten rather than pretend is a branch.
pub fn current_branch(file: &Path) -> Option<String> {
    let root = repo_root(file)?;
    let head = std::fs::read_to_string(root.join(".git").join("HEAD")).ok()?;
    let head = head.trim();

    match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => Some(branch.to_string()),
        // Detached HEAD.
        None if head.len() >= 7 => Some(head[..7].to_string()),
        None => None,
    }
}

/// Who has committed this file, and when it started and last changed.
///
/// One `git log` over a single path — cheap, read-only, and follows renames.
/// Outside a repo (or for an untracked file) this is an empty history rather
/// than an error; the stats block simply omits the git columns.
pub fn history(path: &Path) -> AppResult<FileHistory> {
    let Some(root) = repo_root(path) else {
        return Ok(FileHistory::default());
    };

    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("log")
        .arg("--follow")
        .arg("--format=%an%x00%aI")
        .arg("--")
        .arg(path)
        .output()?;

    if !out.status.success() {
        return Ok(FileHistory::default());
    }
    Ok(parse_log(&String::from_utf8_lossy(&out.stdout)))
}

/// `git log` output is newest-first, one `author\0date` per line.
fn parse_log(text: &str) -> FileHistory {
    let mut counts: Vec<(String, u32)> = Vec::new();
    let mut dates: Vec<String> = Vec::new();

    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut bits = line.split('\0');
        let (Some(author), Some(date)) = (bits.next(), bits.next()) else {
            continue;
        };
        match counts.iter_mut().find(|(a, _)| a == author) {
            Some((_, n)) => *n += 1,
            None => counts.push((author.to_string(), 1)),
        }
        dates.push(date.to_string());
    }

    // Busiest author first — the person to ask about this file.
    counts.sort_by(|a, b| b.1.cmp(&a.1));

    FileHistory {
        commits: dates.len() as u32,
        authors: counts.into_iter().map(|(a, _)| a).collect(),
        first: dates.last().cloned(),
        last: dates.first().cloned(),
    }
}

/// `git blame --line-porcelain`, parsed into one entry per line.
///
/// Called lazily — only when the Blame button is pressed — so opening a file
/// never pays for it.
pub fn blame(path: &Path) -> AppResult<Vec<BlameLine>> {
    let Some(root) = repo_root(path) else {
        return Ok(Vec::new());
    };

    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("blame")
        .arg("--line-porcelain")
        .arg("--")
        .arg(path)
        .output()?;

    if !out.status.success() {
        // Untracked file, or not a git repo after all. Not an error worth
        // interrupting the reader for — the gutter just stays empty.
        return Ok(Vec::new());
    }

    Ok(parse_porcelain(&String::from_utf8_lossy(&out.stdout)))
}

/// Porcelain format: a header line (`<sha> <orig-line> <final-line> [count]`),
/// then `key value` lines, then the source line prefixed with a tab.
fn parse_porcelain(text: &str) -> Vec<BlameLine> {
    let mut lines = Vec::new();
    let mut sha = String::new();
    let mut line_no = 0u32;
    let mut author = String::new();
    let mut ts: i64 = 0;

    for raw in text.lines() {
        if let Some(rest) = raw.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = raw.strip_prefix("author-time ") {
            ts = rest.trim().parse().unwrap_or(0);
        } else if raw.starts_with('\t') {
            // The content line closes the current entry.
            lines.push(BlameLine {
                line: line_no,
                author: std::mem::take(&mut author),
                when: relative_age(ts),
                sha: sha.chars().take(8).collect(),
            });
        } else {
            // Candidate header: "<40-char sha> <orig> <final>[ <count>]".
            let mut parts = raw.split(' ');
            if let (Some(s), Some(_orig), Some(fin)) = (parts.next(), parts.next(), parts.next()) {
                if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                    sha = s.to_string();
                    line_no = fin.parse().unwrap_or(0);
                }
            }
        }
    }
    lines
}

/// "6d", "3mo", "2y" — a gutter has no room for a date.
fn relative_age(author_time: i64) -> String {
    if author_time == 0 {
        return String::new();
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let days = (now - author_time).max(0) / 86_400;
    match days {
        0 => "today".into(),
        1 => "1d".into(),
        2..=30 => format!("{days}d"),
        31..=364 => format!("{}mo", days / 30),
        _ => format!("{}y", days / 365),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory that IS the repository must resolve to itself. Starting at
    /// `parent()` unconditionally made `repo_root` answer "no repository" for
    /// the repository root, which is exactly what the import check passes it.
    #[test]
    fn a_repository_directory_resolves_to_itself() {
        let dir = std::env::temp_dir().join(format!("lgtm-git-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".git")).expect("dirs");
        std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("head");
        std::fs::write(dir.join("a.ex"), "x").expect("file");

        assert_eq!(repo_root(&dir).as_deref(), Some(dir.as_path()), "the directory itself");
        assert_eq!(repo_root(&dir.join("a.ex")).as_deref(), Some(dir.as_path()), "a file in it");
        assert_eq!(current_branch(&dir).as_deref(), Some("main"));

        std::fs::remove_dir_all(&dir).ok();
    }

    const PORCELAIN: &str = "\
a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0 1 1 2
author Carlo Padilla
author-mail <carlo@example.com>
author-time 1700000000
summary initial
filename lib/accounts.ex
\tdefmodule MyApp.Accounts do
a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0 2 2
author Carlo Padilla
author-time 1700000000
\t  @moduledoc \"docs\"
f0e1d2c3b4a5968778695a4b3c2d1e0f9a8b7c6d 3 3 1
author Jane Rivera
author-time 1700086400
\t  def get_user!(id), do: id
";

    #[test]
    fn parses_one_entry_per_line() {
        let lines = parse_porcelain(PORCELAIN);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line, 1);
        assert_eq!(lines[0].author, "Carlo Padilla");
        assert_eq!(lines[2].author, "Jane Rivera");
    }

    #[test]
    fn shortens_the_sha_for_the_gutter() {
        let lines = parse_porcelain(PORCELAIN);
        assert_eq!(lines[0].sha, "a1b2c3d4");
        assert_ne!(lines[0].sha, lines[2].sha);
    }

    #[test]
    fn counts_commits_and_orders_authors_by_volume() {
        let log = "Jane Rivera\u{0}2026-08-10T09:00:00+00:00\n\
                   Carlo Padilla\u{0}2026-06-01T09:00:00+00:00\n\
                   Carlo Padilla\u{0}2025-02-14T09:00:00+00:00\n";
        let h = parse_log(log);

        assert_eq!(h.commits, 3);
        // Carlo has two commits, Jane one — busiest first.
        assert_eq!(h.authors, vec!["Carlo Padilla", "Jane Rivera"]);
        // git log is newest-first, so first/last come from opposite ends.
        assert_eq!(h.last.as_deref(), Some("2026-08-10T09:00:00+00:00"));
        assert_eq!(h.first.as_deref(), Some("2025-02-14T09:00:00+00:00"));
    }

    #[test]
    fn an_empty_log_is_an_empty_history() {
        let h = parse_log("");
        assert_eq!(h.commits, 0);
        assert!(h.authors.is_empty());
        assert!(h.first.is_none() && h.last.is_none());
    }

    #[test]
    fn empty_input_is_empty_output_not_a_panic() {
        assert!(parse_porcelain("").is_empty());
    }

    #[test]
    fn branch_is_read_from_head_text() {
        // The parsing rule, isolated from the filesystem.
        let head = "ref: refs/heads/feature/rate-limit\n";
        assert_eq!(
            head.trim().strip_prefix("ref: refs/heads/"),
            Some("feature/rate-limit")
        );
    }

    #[test]
    fn relative_age_of_zero_is_blank() {
        assert_eq!(relative_age(0), "");
    }
}
