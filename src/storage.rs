use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// Opens the application's SQLite database and creates the initial checklist schema.
pub async fn connect() -> Result<SqlitePool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://checklister.db".to_owned());
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS checklists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS checklist_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            checklist_id INTEGER NOT NULL REFERENCES checklists(id) ON DELETE CASCADE,
            content TEXT NOT NULL,
            is_completed INTEGER NOT NULL DEFAULT 0 CHECK (is_completed IN (0, 1)),
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}
