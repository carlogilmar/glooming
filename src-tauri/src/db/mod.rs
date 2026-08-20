pub mod doc_files;
pub mod docs;
pub mod models;
pub mod projects;

use crate::error::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;

#[cfg(test)]
use std::str::FromStr;

pub const BUNDLE_ID: &str = "com.alertmedia.lgtm";
pub const DB_FILENAME: &str = "lgtm.db";

pub fn default_db_path() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| {
        AppError::BadInput("could not resolve Application Support directory".into())
    })?;
    Ok(base.join(BUNDLE_ID).join(DB_FILENAME))
}

pub async fn open_pool(path: &PathBuf) -> AppResult<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// In-memory pool for tests. One connection so every query shares the same DB.
#[cfg(test)]
pub async fn test_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("connect in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}
