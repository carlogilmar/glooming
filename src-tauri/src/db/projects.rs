//! Folders you have opened, most recent first.

use super::models::Project;
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;

const COLS: &str = "id, path, name, opened_at, \
                    (SELECT COUNT(*) FROM docs d WHERE d.project_id = projects.id) AS gloom_count";

/// Record a folder, or bump it to the top if it is already known.
pub async fn touch(pool: &SqlitePool, path: &str, name: &str) -> AppResult<Project> {
    let now = super::docs::now();
    sqlx::query(
        "INSERT INTO projects (path, name, opened_at) VALUES (?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET opened_at = excluded.opened_at,
                                         name      = excluded.name",
    )
    .bind(path)
    .bind(name)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(sqlx::query_as::<_, Project>(&format!("SELECT {COLS} FROM projects WHERE path = ?"))
        .bind(path)
        .fetch_one(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Project> {
    sqlx::query_as::<_, Project>(&format!("SELECT {COLS} FROM projects WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {id}")))
}

pub async fn recent(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as::<_, Project>(&format!(
        "SELECT {COLS} FROM projects ORDER BY opened_at DESC LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// Remove a project, and everything written about it: its glooms and topics.
///
/// It used to refuse while glooms existed, on the grounds that tidying the recents
/// list must not delete what you wrote — which left a project with glooms no way
/// out at all. Removing is now a deliberate act the UI confirms, naming how many
/// glooms go with it. Only the database is touched; the folder on disk never is.
/// One transaction, children first: a gloom points at its topic and its project,
/// and a topic at its project.
pub async fn forget(pool: &SqlitePool, id: i64) -> AppResult<()> {
    get(pool, id).await?;
    let mut tx = pool.begin().await?;
    for sql in [
        "DELETE FROM docs WHERE project_id = ?",
        "DELETE FROM topics WHERE project_id = ?",
        "DELETE FROM projects WHERE id = ?",
    ] {
        sqlx::query(sql).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn reopening_a_folder_moves_it_up_rather_than_duplicating() {
        let pool = test_pool().await;
        touch(&pool, "/code/my_app", "my_app").await.unwrap();
        touch(&pool, "/code/other", "other").await.unwrap();
        touch(&pool, "/code/my_app", "my_app").await.unwrap();

        let all = recent(&pool, 10).await.unwrap();
        assert_eq!(all.len(), 2, "one row per path");
        assert_eq!(all[0].path, "/code/my_app", "most recent first");
    }

    #[tokio::test]
    async fn forgets_a_folder() {
        let pool = test_pool().await;
        let p = touch(&pool, "/code/my_app", "my_app").await.unwrap();
        forget(&pool, p.id).await.unwrap();
        assert!(recent(&pool, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_project_counts_its_glooms() {
        let pool = test_pool().await;
        let p = touch(&pool, "/code/my_app", "my_app").await.unwrap();
        assert_eq!(p.gloom_count, 0);
        crate::db::docs::create(&pool, p.id, "One", "").await.unwrap();
        crate::db::docs::create(&pool, p.id, "Two", "").await.unwrap();
        assert_eq!(recent(&pool, 10).await.unwrap()[0].gloom_count, 2);
    }

    /// Removing a project takes its glooms and topics with it — and only its own.
    #[tokio::test]
    async fn forgetting_a_project_deletes_its_glooms_and_topics() {
        let pool = test_pool().await;
        let p = touch(&pool, "/code/my_app", "my_app").await.unwrap();
        let other = touch(&pool, "/code/other", "other").await.unwrap();
        crate::db::docs::create(&pool, p.id, "One", "").await.unwrap();
        crate::db::docs::create(&pool, p.id, "Two", "").await.unwrap();
        crate::db::docs::create(&pool, other.id, "Kept", "").await.unwrap();
        crate::db::topics::create(&pool, p.id, "Billing").await.unwrap();

        forget(&pool, p.id).await.unwrap();

        let left = recent(&pool, 10).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].path, "/code/other");
        assert_eq!(left[0].gloom_count, 1, "another project's glooms are untouched");
        let (docs,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM docs").fetch_one(&pool).await.unwrap();
        let (topics,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM topics").fetch_one(&pool).await.unwrap();
        assert_eq!((docs, topics), (1, 0));
    }
}
