use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

/// Opens the configured persistent SQLite database and applies pending migrations.
pub async fn connect() -> Result<SqlitePool, Box<dyn std::error::Error + Send + Sync>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://checklister.db".to_owned());
    connect_with_url(&database_url).await
}

/// Connects to a specific SQLite database. This is also used by isolated tests.
pub async fn connect_with_url(
    database_url: &str,
) -> Result<SqlitePool, Box<dyn std::error::Error + Send + Sync>> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(5))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    prepare_legacy_database(&pool).await?;
    MIGRATOR.run(&pool).await?;
    migrate_unversioned_database(&pool).await?;
    Ok(pool)
}

/// Makes the pre-migration index creation safe for the previous inline schema.
async fn prepare_legacy_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let has_checks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'checks'",
    )
    .fetch_one(pool)
    .await?;
    if has_checks > 0 {
        let has_position: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('checks') WHERE name = 'position'",
        )
        .fetch_one(pool)
        .await?;
        if has_position == 0 {
            sqlx::query("ALTER TABLE checks ADD COLUMN position INTEGER NOT NULL DEFAULT 0")
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Adopts databases created before SQLx migrations were introduced.
async fn migrate_unversioned_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let has_name: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('checklists') WHERE name = 'name'",
    )
    .fetch_one(pool)
    .await?;
    if has_name == 0 {
        let has_title: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('checklists') WHERE name = 'title'",
        )
        .fetch_one(pool)
        .await?;
        if has_title > 0 {
            sqlx::query("ALTER TABLE checklists ADD COLUMN name TEXT")
                .execute(pool)
                .await?;
            sqlx::query("UPDATE checklists SET name = title WHERE name IS NULL")
                .execute(pool)
                .await?;
        }
    }

    let has_legacy_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'checklist_items'",
    )
    .fetch_one(pool)
    .await?;
    if has_legacy_items > 0 {
        sqlx::query(
            "INSERT OR IGNORE INTO checks
                (id, checklist_id, status, title, description, position, created_at)
             SELECT id, checklist_id, is_completed, content, '', position,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 946684800 + position, 'unixepoch')
             FROM checklist_items
             WHERE EXISTS (
                 SELECT 1 FROM checklists WHERE checklists.id = checklist_items.checklist_id
             )",
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}
