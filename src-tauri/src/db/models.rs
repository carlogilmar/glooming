use serde::{Deserialize, Serialize};

/// A full doc, including both large text columns.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub lang: String,
    pub title: String,
    pub branch: Option<String>,
    pub label: Option<String>,
    pub markdown: String,
    pub source: String,
    pub source_sha: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Everything the library list and the reopen chooser need — deliberately
/// without `markdown` or `source`, so listing never drags text around.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DocSummary {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub lang: String,
    pub title: String,
    pub branch: Option<String>,
    pub label: Option<String>,
    pub source_sha: String,
    pub created_at: String,
    pub updated_at: String,
    /// How many files this reading covers. 1 for every doc written before
    /// readings could span several.
    pub file_count: i64,
}

/// What the frontend receives when a file is opened: the source, its outline,
/// and any docs already written about this path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedFile {
    pub path: String,
    pub filename: String,
    pub source: String,
    pub source_sha: String,
    pub lang: Option<String>,
    pub outline: Option<crate::parse::Outline>,
    /// Prefilled from the nearest `.git/HEAD`; null outside a repo.
    pub branch: Option<String>,
    /// True when a `.git` directory exists — gates the Blame button.
    pub has_git: bool,
    pub existing: Vec<DocSummary>,
}

/// One file in a reading, as stored.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    pub id: i64,
    pub doc_id: i64,
    pub path: String,
    pub filename: String,
    pub lang: String,
    pub source: String,
    pub source_sha: String,
    pub position: i64,
    pub added_at: String,
}

/// One file in a reading, as the UI needs it: the source to display, its
/// outline, and whether the code has moved since it was read.
///
/// `source` is what to *show* — disk when the file is readable, the stored
/// snapshot when it is not. That is the whole point of keeping a snapshot per
/// file: a reading survives one of its files being deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingFile {
    pub path: String,
    pub filename: String,
    pub lang: Option<String>,
    pub source: String,
    /// sha of `source` — the thing on screen.
    pub source_sha: String,
    /// sha of the snapshot taken when this file joined the reading.
    pub snapshot_sha: String,
    /// The file on disk has changed since it was read.
    pub stale: bool,
    /// Not on disk any more; `source` is the snapshot.
    pub missing: bool,
    pub outline: Option<crate::parse::Outline>,
    pub has_git: bool,
    pub branch: Option<String>,
    /// The file this doc was seeded from. Its module owns `lgtm:functions`, and
    /// it cannot be removed from the reading.
    pub origin: bool,
}

/// A whole reading in one payload: the doc plus every file it covers.
///
/// One round trip rather than N, for the same reason `open_file` returns source
/// and outline and existing docs together — the UI renders from one payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub doc: Doc,
    pub files: Vec<ReadingFile>,
}

/// One line of `git blame` output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line: u32,
    pub author: String,
    pub when: String,
    pub sha: String,
}

/// What git knows about a file's life: who has touched it, and when it started
/// and last changed. Read-only, and empty outside a repo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistory {
    pub commits: u32,
    /// Distinct authors, most commits first.
    pub authors: Vec<String>,
    /// ISO-8601 date of the first commit that touched this file.
    pub first: Option<String>,
    pub last: Option<String>,
}

/// A folder you search within. Deliberately thin: a project is a path, not a
/// scanned index or a set of member files.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub opened_at: String,
}

/// One candidate in the file picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    /// Absolute path, for opening.
    pub path: String,
    /// Path relative to the project root — what you actually search and read.
    pub rel: String,
    pub name: String,
}
