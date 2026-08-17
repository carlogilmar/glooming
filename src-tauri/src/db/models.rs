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
