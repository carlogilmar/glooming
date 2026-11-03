pub mod docs;
pub mod hub;
pub mod files;
pub mod projects;

use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
}
