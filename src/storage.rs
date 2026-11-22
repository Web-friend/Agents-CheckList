use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// Opens the application's SQLite database and ensures the checklist schema exists.
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
            name TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    // Migrate databases created by the earlier title-based checklist schema.
    let has_name: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('checklists') WHERE name = 'name'",
    )
    .fetch_one(&pool)
    .await?;
    if has_name == 0 {
        sqlx::query("ALTER TABLE checklists ADD COLUMN name TEXT")
            .execute(&pool)
            .await?;
        sqlx::query("UPDATE checklists SET name = title WHERE name IS NULL")
            .execute(&pool)
            .await?;
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            checklist_id INTEGER NOT NULL REFERENCES checklists(id) ON DELETE CASCADE,
            status BOOLEAN NOT NULL DEFAULT FALSE CHECK (status IN (FALSE, TRUE)),
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            position INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await?;

    let has_position: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('checks') WHERE name = 'position'",
    )
    .fetch_one(&pool)
    .await?;
    if has_position == 0 {
        sqlx::query("ALTER TABLE checks ADD COLUMN position INTEGER NOT NULL DEFAULT 0")
            .execute(&pool)
            .await?;
    }

    // Preserve items created by the previous version of the app.
    let has_legacy_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'checklist_items'",
    )
    .fetch_one(&pool)
    .await?;
    if has_legacy_items > 0 {
        sqlx::query(
            "INSERT OR IGNORE INTO checks (id, checklist_id, status, title, description, position)
             SELECT id, checklist_id, is_completed, content, '', position FROM checklist_items",
        )
        .execute(&pool)
        .await?;
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS checks_checklist_order
         ON checks (checklist_id, position, id)",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}
