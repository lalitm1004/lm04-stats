use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::Arc;

use crate::ENV_CONFIG;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<SqlitePool>,
}

pub async fn create_db_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&ENV_CONFIG.database_url)
        .await
        .expect("Failed to connect to database")
}
