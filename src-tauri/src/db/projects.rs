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

/// Drop a folder from the recents.
///
/// Refused while glooms belong to it: forgetting a folder is tidying a list, and
/// it must never be the gesture that throws away what you wrote about it.
pub async fn forget(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let p = get(pool, id).await?;
    if p.gloom_count > 0 {
        return Err(AppError::BadInput(format!(
            "{} has {} gloom{} — delete {} first to forget the folder",
            p.name,
            p.gloom_count,
            if p.gloom_count == 1 { "" } else { "s" },
            if p.gloom_count == 1 { "it" } else { "them" },
        )));
    }
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

    #[tokio::test]
    async fn a_project_counts_its_glooms() {
        let pool = test_pool().await;
        let p = touch(&pool, "/code/my_app", "my_app").await.unwrap();
        assert_eq!(p.gloom_count, 0);
        crate::db::docs::create(&pool, p.id, "One", "").await.unwrap();
        crate::db::docs::create(&pool, p.id, "Two", "").await.unwrap();
        assert_eq!(recent(&pool, 10).await.unwrap()[0].gloom_count, 2);
    }

    /// Tidying the recents list must never delete what you wrote.
    #[tokio::test]
    async fn a_folder_with_glooms_cannot_be_forgotten() {
        let pool = test_pool().await;
        let p = touch(&pool, "/code/my_app", "my_app").await.unwrap();
        crate::db::docs::create(&pool, p.id, "One", "").await.unwrap();

        let err = forget(&pool, p.id).await.expect_err("refuses");
        assert!(err.to_string().contains("1 gloom"), "got: {err}");
        assert_eq!(recent(&pool, 10).await.unwrap().len(), 1, "still there");
    }
}
