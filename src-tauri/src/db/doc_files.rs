//! The files a reading covers.
//!
//! `docs.path` remains the **origin** — the file the doc was seeded from — and
//! is also the first row here. Everything else was opened during the review and
//! joined the reading without being seeded: adding a file contributes source to
//! read and vocabulary to reference, and nothing else.

use super::docs::now;
use super::models::DocFile;
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

const COLS: &str = "id, doc_id, path, filename, lang, source, source_sha, position, added_at";

pub async fn list(pool: &SqlitePool, doc_id: i64) -> AppResult<Vec<DocFile>> {
    let sql = format!("SELECT {COLS} FROM doc_files WHERE doc_id = ? ORDER BY position, id");
    Ok(sqlx::query_as::<_, DocFile>(&sql)
        .bind(doc_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, doc_id: i64, path: &str) -> AppResult<Option<DocFile>> {
    let sql = format!("SELECT {COLS} FROM doc_files WHERE doc_id = ? AND path = ?");
    Ok(sqlx::query_as::<_, DocFile>(&sql)
        .bind(doc_id)
        .bind(path)
        .fetch_optional(pool)
        .await?)
}

/// Add a file to a reading, or return the row it already has.
///
/// Idempotent on purpose: opening the same file twice during a review is
/// completely normal, and it must not re-snapshot — that would silently discard
/// the staleness you were about to be told about.
#[allow(clippy::too_many_arguments)]
pub async fn add(
    pool: &SqlitePool,
    doc_id: i64,
    path: &str,
    filename: &str,
    lang: &str,
    source: &str,
    source_sha: &str,
) -> AppResult<DocFile> {
    if let Some(existing) = get(pool, doc_id, path).await? {
        return Ok(existing);
    }

    // Append. Positions are never renumbered, so removing a file in the middle
    // leaves the others where they were in the strip.
    let next: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM doc_files WHERE doc_id = ?",
    )
    .bind(doc_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO doc_files (doc_id, path, filename, lang, source, source_sha,
                                position, added_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(doc_id)
    .bind(path)
    .bind(filename)
    .bind(lang)
    .bind(source)
    .bind(source_sha)
    .bind(next)
    .bind(now())
    .execute(pool)
    .await?;

    get(pool, doc_id, path)
        .await?
        .ok_or_else(|| AppError::Other(format!("could not read back {path}")))
}

/// Drop a file from a reading.
///
/// The origin is refused: it is what `docs.path` points at, what the library
/// groups by, and whose module owns the `lgtm:functions` block. Removing it
/// would leave a reading that is about nothing in particular — delete the whole
/// doc instead, which asks first and says what is lost.
pub async fn remove(pool: &SqlitePool, doc_id: i64, path: &str, origin: &str) -> AppResult<()> {
    if path == origin {
        return Err(AppError::BadInput(
            "that is the file this reading started from — delete the reading instead".into(),
        ));
    }
    sqlx::query("DELETE FROM doc_files WHERE doc_id = ? AND path = ?")
        .bind(doc_id)
        .bind(path)
        .execute(pool)
        .await?;
    Ok(())
}

/// Re-snapshot one file, so it stops reading as stale.
pub async fn resnapshot(
    pool: &SqlitePool,
    doc_id: i64,
    path: &str,
    source: &str,
    source_sha: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE doc_files SET source = ?, source_sha = ? WHERE doc_id = ? AND path = ?",
    )
    .bind(source)
    .bind(source_sha)
    .bind(doc_id)
    .bind(path)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{docs as docs_db, test_pool};

    async fn reading(pool: &SqlitePool) -> i64 {
        let doc = docs_db::create(
            pool,
            "/app/accounts.ex",
            "accounts.ex",
            "elixir",
            "MyApp.Accounts",
            Some("main"),
            "# hi",
            "def a",
            "sha-a",
        )
        .await
        .expect("create");
        // `create_doc` does this for real; the DB layer is tested on its own.
        add(
            pool, doc.id, "/app/accounts.ex", "accounts.ex", "elixir", "def a", "sha-a",
        )
        .await
        .expect("origin");
        doc.id
    }

    #[tokio::test]
    async fn a_fresh_doc_is_a_one_file_reading() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        let files = list(&pool, id).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/app/accounts.ex");
        assert_eq!(files[0].position, 0);
    }

    #[tokio::test]
    async fn files_come_back_in_the_order_they_were_added() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        for (p, n) in [("/app/billing.ex", "billing.ex"), ("/app/mailer.ex", "mailer.ex")] {
            add(&pool, id, p, n, "elixir", "def b", "sha-b").await.unwrap();
        }
        let paths: Vec<_> = list(&pool, id).await.unwrap().into_iter().map(|f| f.path).collect();
        assert_eq!(paths, ["/app/accounts.ex", "/app/billing.ex", "/app/mailer.ex"]);
    }

    /// Opening the same file twice mid-review must not re-snapshot it — that
    /// would throw away the staleness you were about to be shown.
    #[tokio::test]
    async fn adding_twice_keeps_the_first_snapshot() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        add(&pool, id, "/app/billing.ex", "billing.ex", "elixir", "old", "sha-old")
            .await
            .unwrap();
        let again = add(
            &pool, id, "/app/billing.ex", "billing.ex", "elixir", "new", "sha-new",
        )
        .await
        .unwrap();

        assert_eq!(again.source, "old", "snapshot must survive a re-open");
        assert_eq!(again.source_sha, "sha-old");
        assert_eq!(list(&pool, id).await.unwrap().len(), 2, "and not duplicate");
    }

    #[tokio::test]
    async fn removing_a_file_leaves_the_others_where_they_were() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        add(&pool, id, "/app/billing.ex", "billing.ex", "elixir", "b", "sb").await.unwrap();
        add(&pool, id, "/app/mailer.ex", "mailer.ex", "elixir", "m", "sm").await.unwrap();

        remove(&pool, id, "/app/billing.ex", "/app/accounts.ex").await.unwrap();

        let files = list(&pool, id).await.unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].path, "/app/mailer.ex");
        assert_eq!(files[1].position, 2, "positions are not renumbered");
    }

    #[tokio::test]
    async fn the_origin_cannot_be_removed() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        let err = remove(&pool, id, "/app/accounts.ex", "/app/accounts.ex").await;
        assert!(err.is_err(), "removing the origin must be refused");
        assert_eq!(list(&pool, id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn resnapshotting_clears_the_stale_sha() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        resnapshot(&pool, id, "/app/accounts.ex", "def a2", "sha-a2").await.unwrap();
        let f = get(&pool, id, "/app/accounts.ex").await.unwrap().unwrap();
        assert_eq!(f.source, "def a2");
        assert_eq!(f.source_sha, "sha-a2");
    }

    /// Deleting a reading takes its file rows with it — the snapshots are part
    /// of what the library's delete confirmation says is lost.
    #[tokio::test]
    async fn deleting_the_doc_takes_its_files() {
        let pool = test_pool().await;
        let id = reading(&pool).await;
        add(&pool, id, "/app/billing.ex", "billing.ex", "elixir", "b", "sb").await.unwrap();
        docs_db::delete(&pool, id).await.unwrap();
        assert!(list(&pool, id).await.unwrap().is_empty());
    }
}
