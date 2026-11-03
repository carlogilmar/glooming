use serde::{Deserialize, Serialize};

/// A gloom: a note in a project's knowledge base.
///
/// Prose and nothing else. There is no snapshot of the code and no list of the
/// files it covers — the code is always read live from disk, and the modules a
/// gloom is about are whatever its prose names.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    pub id: i64,
    pub project_id: i64,
    /// Empty until you name it.
    pub title: String,
    /// Tags, comma-separated.
    pub label: Option<String>,
    pub markdown: String,
    pub created_at: String,
    pub updated_at: String,
    /// Its topic in the Hub; `None` is Unsorted.
    pub topic_id: Option<i64>,
    /// Its place within that topic.
    pub position: i64,
}

/// A section of the Hub: a name and a place in the reading order.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub position: i64,
    pub created_at: String,
}

/// Everything the Hub draws, in one payload: the topics in order, and every
/// gloom with its prose — the Hub reads the prose to say which modules a gloom
/// walks and whether its references still resolve, so a summary would only mean
/// a second round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hub {
    pub topics: Vec<Topic>,
    pub glooms: Vec<Doc>,
}

/// Where one gloom goes when the Hub is rearranged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub id: i64,
    pub topic_id: Option<i64>,
}

/// What the sidebar and the library list — deliberately without `markdown`, so
/// listing never drags text around.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DocSummary {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A file as it is on disk right now: its source and its outline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedFile {
    pub path: String,
    pub filename: String,
    pub source: String,
    pub lang: Option<String>,
    pub outline: Option<crate::parse::Outline>,
    /// Read from the nearest `.git/HEAD`; null outside a repo.
    pub branch: Option<String>,
    /// True when a `.git` directory exists — gates the Blame button.
    pub has_git: bool,
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

/// A codebase you explore. A path, a name, and the glooms written about it.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub opened_at: String,
    /// How many glooms belong to it — what home shows beside the name.
    pub gloom_count: i64,
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
