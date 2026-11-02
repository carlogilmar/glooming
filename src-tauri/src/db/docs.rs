//! Gloom persistence. Plain sqlx; the commands layer stays thin.

use super::models::{Doc, DocSummary};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

const SUMMARY_COLS: &str = "id, project_id, title, label, created_at, updated_at";

pub fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub async fn create(
    pool: &SqlitePool,
    project_id: i64,
    title: &str,
    markdown: &str,
) -> AppResult<Doc> {
    let ts = now();
    let id = sqlx::query(
        "INSERT INTO docs (project_id, title, markdown, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(project_id)
    .bind(title)
    .bind(markdown)
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
        .ok_or_else(|| AppError::NotFound(format!("gloom {id}")))
}

/// Partial update — only the fields actually supplied are touched, so autosave
/// can send just the markdown without clobbering a title edit.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    markdown: Option<&str>,
    title: Option<&str>,
    label: Option<&str>,
) -> AppResult<Doc> {
    sqlx::query(
        "UPDATE docs
            SET markdown   = COALESCE(?, markdown),
                title      = COALESCE(?, title),
                label      = COALESCE(?, label),
                updated_at = ?
          WHERE id = ?",
    )
    .bind(markdown)
    .bind(title)
    .bind(label)
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

/// A project's glooms, most recently touched first. `query` matches the name
/// and the tags — a tag you cannot search by is decoration — and the prose,
/// because "the one where I worked out the retry" is usually remembered by what
/// it says rather than what it is called.
pub async fn list(
    pool: &SqlitePool,
    project_id: i64,
    query: Option<&str>,
    limit: i64,
) -> AppResult<Vec<DocSummary>> {
    match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{q}%");
            let sql = format!(
                "SELECT {SUMMARY_COLS} FROM docs
                  WHERE project_id = ?
                    AND (title LIKE ? OR COALESCE(label, '') LIKE ? OR markdown LIKE ?)
                  ORDER BY updated_at DESC LIMIT ?"
            );
            Ok(sqlx::query_as::<_, DocSummary>(&sql)
                .bind(project_id)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(limit)
                .fetch_all(pool)
                .await?)
        }
        None => {
            let sql = format!(
                "SELECT {SUMMARY_COLS} FROM docs WHERE project_id = ?
                  ORDER BY updated_at DESC LIMIT ?"
            );
            Ok(sqlx::query_as::<_, DocSummary>(&sql)
                .bind(project_id)
                .bind(limit)
                .fetch_all(pool)
                .await?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{projects, test_pool};

    async fn project(pool: &SqlitePool, path: &str) -> i64 {
        projects::touch(pool, path, "app").await.expect("project").id
    }

    #[tokio::test]
    async fn creates_and_reads_back() {
        let pool = test_pool().await;
        let p = project(&pool, "/code/app").await;
        let doc = create(&pool, p, "How billing works", "prose").await.unwrap();
        assert_eq!(doc.title, "How billing works");
        assert_eq!(doc.project_id, p);
        assert_eq!(get(&pool, doc.id).await.unwrap().markdown, "prose");
    }

    #[tokio::test]
    async fn update_leaves_untouched_fields_alone() {
        let pool = test_pool().await;
        let p = project(&pool, "/code/app").await;
        let doc = create(&pool, p, "Billing", "# hi").await.unwrap();

        let updated = update(&pool, doc.id, Some("# new"), None, None).await.unwrap();
        assert_eq!(updated.markdown, "# new");
        assert_eq!(updated.title, "Billing", "title must survive");
    }

    /// A project's glooms are its own: two codebases never share a sidebar.
    #[tokio::test]
    async fn a_project_lists_only_its_own_glooms() {
        let pool = test_pool().await;
        let a = project(&pool, "/code/a").await;
        let b = project(&pool, "/code/b").await;
        create(&pool, a, "In A", "").await.unwrap();
        create(&pool, b, "In B", "").await.unwrap();

        let found = list(&pool, a, None, 50).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "In A");
    }

    /// Tags live in `label` as a comma-separated list, and are searched — a tag
    /// you cannot find by is a decoration. So is the prose itself.
    #[tokio::test]
    async fn search_matches_the_name_the_tags_and_the_prose() {
        let pool = test_pool().await;
        let p = project(&pool, "/code/app").await;
        let id = create(&pool, p, "Ingest", "the retry has no backoff").await.unwrap().id;
        create(&pool, p, "Billing", "").await.unwrap();
        update(&pool, id, None, None, Some("perf, weather")).await.unwrap();

        assert_eq!(list(&pool, p, Some("Ingest"), 10).await.unwrap().len(), 1);
        assert_eq!(list(&pool, p, Some("weather"), 10).await.unwrap().len(), 1);
        assert_eq!(list(&pool, p, Some("backoff"), 10).await.unwrap().len(), 1);
        assert!(list(&pool, p, Some("nothing-like-this"), 10).await.unwrap().is_empty());
    }

    /// Saving into a gloom that has been deleted must fail loudly.
    ///
    /// It is why the library tells the shell what it deleted: leaving the gloom
    /// open means the next autosave lands here, and the error surfaces seconds
    /// later with nothing on screen to connect it to the delete.
    #[tokio::test]
    async fn saving_to_a_deleted_doc_is_an_error_not_a_silent_no_op() {
        let pool = test_pool().await;
        let p = project(&pool, "/code/app").await;
        let doc = create(&pool, p, "Billing", "").await.unwrap();
        delete(&pool, doc.id).await.unwrap();

        assert!(
            update(&pool, doc.id, Some("# written after the delete"), None, None)
                .await
                .is_err(),
            "an UPDATE matching no rows must not report success"
        );
    }
}
