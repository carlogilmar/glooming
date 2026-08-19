//! Folders you have opened, most recent first.

use super::models::Project;
use crate::error::AppResult;
use sqlx::SqlitePool;

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

    Ok(sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE path = ?")
        .bind(path)
        .fetch_one(pool)
        .await?)
}

pub async fn recent(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Project>> {
    Ok(
        sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY opened_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn forget(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
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
}
