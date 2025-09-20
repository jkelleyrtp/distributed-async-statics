use const_to_static_table::{Lazy, initialize};
use sqlx::{Executor, SqlitePool};

static DB_POOL: Lazy<SqlitePool> =
    Lazy::new(|| async { SqlitePool::connect("sqlite::memory:").await.unwrap() });

#[tokio::main]
async fn main() {
    // Initialize all the static variables
    initialize().await;

    // And then use the database pool
    DB_POOL
        .execute(sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        ))
        .await;
}
