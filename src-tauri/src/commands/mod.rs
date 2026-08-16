pub mod docs;
pub mod files;

use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
}
