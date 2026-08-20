//! Doc persistence. Plain sqlx; the commands layer stays thin.

use super::models::{Doc, DocSummary};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

// The file count comes along as a subquery rather than a second round trip: the
// library lists a *reading*, and "accounts.ex" is a misleading name for one that
// covers four files.
const SUMMARY_COLS: &str = "id, path, filename, lang, title, branch, label, \
                            source_sha, created_at, updated_at, \
                            (SELECT COUNT(*) FROM doc_files df WHERE df.doc_id = docs.id) \
                              AS file_count";

pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &SqlitePool,
    path: &str,
    filename: &str,
    lang: &str,
    title: &str,
    branch: Option<&str>,
    markdown: &str,
    source: &str,
    source_sha: &str,
) -> AppResult<Doc> {
    let ts = now();
    let id = sqlx::query(
        "INSERT INTO docs (path, filename, lang, title, branch, markdown, source,
                           source_sha, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(path)
    .bind(filename)
    .bind(lang)
    .bind(title)
    .bind(branch)
    .bind(markdown)
    .bind(source)
    .bind(source_sha)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?
    .last_insert_rowid();

    get(pool, id).await
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Doc> {
    sqlx::query_as::<_, Doc>("SELECT * FROM docs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("doc {id}")))
}

/// Partial update — only the fields actually supplied are touched, so autosave
/// can send just the markdown without clobbering a title edit.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    markdown: Option<&str>,
    title: Option<&str>,
    branch: Option<&str>,
    label: Option<&str>,
) -> AppResult<Doc> {
    sqlx::query(
        "UPDATE docs
            SET markdown   = COALESCE(?, markdown),
                title      = COALESCE(?, title),
                branch     = COALESCE(?, branch),
                label      = COALESCE(?, label),
                updated_at = ?
          WHERE id = ?",
    )
    .bind(markdown)
    .bind(title)
    .bind(branch)
    .bind(label)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

/// Re-snapshot the source after a reconcile, so the doc stops reading as stale.
pub async fn update_source(
    pool: &SqlitePool,
    id: i64,
    source: &str,
    source_sha: &str,
    markdown: &str,
) -> AppResult<Doc> {
    sqlx::query(
        "UPDATE docs SET source = ?, source_sha = ?, markdown = ?, updated_at = ?
          WHERE id = ?",
    )
    .bind(source)
    .bind(source_sha)
    .bind(markdown)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM docs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Readings that already cover this path.
///
/// Matches on the reading's *set*, not just its origin: during a review you open
/// billing.ex on Tuesday as part of a reading of accounts.ex, and on Wednesday
/// you open billing.ex first. Offering only docs whose `path` is billing.ex
/// would hide the reading you actually want and quietly start a duplicate.
pub async fn for_path(pool: &SqlitePool, path: &str) -> AppResult<Vec<DocSummary>> {
    let sql = format!(
        "SELECT {SUMMARY_COLS} FROM docs
          WHERE path = ?
             OR id IN (SELECT doc_id FROM doc_files WHERE path = ?)
          ORDER BY updated_at DESC"
    );
    Ok(sqlx::query_as::<_, DocSummary>(&sql)
        .bind(path)
        .bind(path)
        .fetch_all(pool)
        .await?)
}

/// The library list. `query` matches title, filename, path and branch.
pub async fn list(pool: &SqlitePool, query: Option<&str>, limit: i64) -> AppResult<Vec<DocSummary>> {
    match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{q}%");
            let sql = format!(
                "SELECT {SUMMARY_COLS} FROM docs
                  WHERE title LIKE ? OR filename LIKE ? OR path LIKE ?
                     OR COALESCE(branch, '') LIKE ?
                  ORDER BY updated_at DESC LIMIT ?"
            );
            Ok(sqlx::query_as::<_, DocSummary>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .fetch_all(pool)
                .await?)
        }
        None => {
            let sql = format!("SELECT {SUMMARY_COLS} FROM docs ORDER BY updated_at DESC LIMIT ?");
            Ok(sqlx::query_as::<_, DocSummary>(&sql)
                .bind(limit)
                .fetch_all(pool)
                .await?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed(pool: &SqlitePool, path: &str, title: &str) -> Doc {
        create(
            pool, path, "accounts.ex", "elixir", title, Some("main"), "# hi", "def x", "sha1",
        )
        .await
        .expect("create")
    }

    #[tokio::test]
    async fn creates_and_reads_back() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "MyApp.Accounts").await;
        assert_eq!(doc.title, "MyApp.Accounts");
        assert_eq!(get(&pool, doc.id).await.unwrap().markdown, "# hi");
    }

    #[tokio::test]
    async fn update_leaves_untouched_fields_alone() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "MyApp.Accounts").await;

        let updated = update(&pool, doc.id, Some("# new"), None, None, None)
            .await
            .unwrap();
        assert_eq!(updated.markdown, "# new");
        assert_eq!(updated.title, "MyApp.Accounts", "title must survive");
        assert_eq!(updated.branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn finds_existing_docs_for_a_path() {
        let pool = test_pool().await;
        seed(&pool, "/a/accounts.ex", "First").await;
        seed(&pool, "/b/other.ex", "Other").await;

        let found = for_path(&pool, "/a/accounts.ex").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "First");
    }

    /// A file that joined a reading later is still a way back into it.
    #[tokio::test]
    async fn finds_a_reading_by_any_of_its_files() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "Signup flow").await;
        crate::db::doc_files::add(
            &pool, doc.id, "/a/billing.ex", "billing.ex", "elixir", "def b", "sb",
        )
        .await
        .unwrap();

        let found = for_path(&pool, "/a/billing.ex").await.unwrap();
        assert_eq!(found.len(), 1, "the reading that covers it");
        assert_eq!(found[0].title, "Signup flow");
        assert_eq!(found[0].path, "/a/accounts.ex", "reported by its origin");
    }

    #[tokio::test]
    async fn search_matches_title_and_branch() {
        let pool = test_pool().await;
        seed(&pool, "/a/accounts.ex", "MyApp.Accounts").await;
        seed(&pool, "/b/other.ex", "MyApp.Billing").await;

        assert_eq!(list(&pool, Some("Billing"), 50).await.unwrap().len(), 1);
        assert_eq!(list(&pool, Some("main"), 50).await.unwrap().len(), 2);
        assert_eq!(list(&pool, None, 50).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_summary_carries_its_file_count() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "Signup flow").await;
        crate::db::doc_files::add(
            &pool, doc.id, "/a/accounts.ex", "accounts.ex", "elixir", "a", "sa",
        )
        .await
        .unwrap();
        crate::db::doc_files::add(
            &pool, doc.id, "/a/billing.ex", "billing.ex", "elixir", "b", "sb",
        )
        .await
        .unwrap();

        let found = list(&pool, None, 50).await.unwrap();
        assert_eq!(found[0].file_count, 2);
    }

    /// Saving into a reading that has been deleted must fail loudly.
    ///
    /// It is why the library tells the shell what it deleted: leaving the reading
    /// open means the next autosave lands here, and the error surfaces seconds
    /// later with nothing on screen to connect it to the delete.
    #[tokio::test]
    async fn saving_to_a_deleted_doc_is_an_error_not_a_silent_no_op() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "MyApp.Accounts").await;
        delete(&pool, doc.id).await.unwrap();

        assert!(
            update(&pool, doc.id, Some("# written after the delete"), None, None, None)
                .await
                .is_err(),
            "an UPDATE matching no rows must not report success"
        );
    }

    #[tokio::test]
    async fn deletes() {
        let pool = test_pool().await;
        let doc = seed(&pool, "/a/accounts.ex", "MyApp.Accounts").await;
        delete(&pool, doc.id).await.unwrap();
        assert!(get(&pool, doc.id).await.is_err());
    }
}
