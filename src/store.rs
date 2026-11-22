use serde::Serialize;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Serialize)]
pub struct Checklist {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub id: i64,
    pub checklist_id: i64,
    pub status: bool,
    pub title: String,
    pub description: String,
    pub position: i64,
}

#[derive(Debug, Serialize)]
pub struct ChecklistDetails {
    pub checklist: Checklist,
    pub checks: Vec<Check>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_checklist(&self, name: &str) -> Result<Checklist, StoreError> {
        let result = sqlx::query("INSERT INTO checklists (name) VALUES (?)")
            .bind(name)
            .execute(&self.pool)
            .await?;
        self.get_checklist(result.last_insert_rowid()).await
    }

    pub async fn list_checklists(&self) -> Result<Vec<Checklist>, StoreError> {
        let rows = sqlx::query("SELECT id, name FROM checklists ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(checklist_from_row).collect()
    }

    pub async fn get_checklist(&self, id: i64) -> Result<Checklist, StoreError> {
        let row = sqlx::query("SELECT id, name FROM checklists WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::NotFound("checklist"))?;
        checklist_from_row(&row)
    }

    pub async fn show_checklist(&self, id: i64) -> Result<ChecklistDetails, StoreError> {
        let checklist = self.get_checklist(id).await?;
        let checks = self.list_checks(id, false).await?;
        Ok(ChecklistDetails { checklist, checks })
    }

    pub async fn delete_checklist(&self, id: i64) -> Result<Checklist, StoreError> {
        let checklist = self.get_checklist(id).await?;
        sqlx::query("DELETE FROM checklists WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(checklist)
    }

    pub async fn add_check(
        &self,
        checklist_id: i64,
        title: &str,
        description: &str,
    ) -> Result<Check, StoreError> {
        self.get_checklist(checklist_id).await?;
        let position: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM checks WHERE checklist_id = ?",
        )
        .bind(checklist_id)
        .fetch_one(&self.pool)
        .await?;
        let result = sqlx::query(
            "INSERT INTO checks (checklist_id, title, description, position)
             VALUES (?, ?, ?, ?)",
        )
        .bind(checklist_id)
        .bind(title)
        .bind(description)
        .bind(position)
        .execute(&self.pool)
        .await?;
        self.get_check(checklist_id, result.last_insert_rowid()).await
    }

    pub async fn list_checks(
        &self,
        checklist_id: i64,
        undone_only: bool,
    ) -> Result<Vec<Check>, StoreError> {
        self.get_checklist(checklist_id).await?;
        let query = if undone_only {
            "SELECT id, checklist_id, status, title, description, position
             FROM checks WHERE checklist_id = ? AND status = FALSE ORDER BY position, id"
        } else {
            "SELECT id, checklist_id, status, title, description, position
             FROM checks WHERE checklist_id = ? ORDER BY position, id"
        };
        let rows = sqlx::query(query)
            .bind(checklist_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(check_from_row).collect()
    }

    pub async fn set_check_status(
        &self,
        checklist_id: i64,
        check_id: i64,
        status: bool,
    ) -> Result<Check, StoreError> {
        let result = sqlx::query(
            "UPDATE checks SET status = ? WHERE checklist_id = ? AND id = ?",
        )
        .bind(status)
        .bind(checklist_id)
        .bind(check_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound("check"));
        }
        self.get_check(checklist_id, check_id).await
    }

    async fn get_check(&self, checklist_id: i64, check_id: i64) -> Result<Check, StoreError> {
        let row = sqlx::query(
            "SELECT id, checklist_id, status, title, description, position
             FROM checks WHERE checklist_id = ? AND id = ?",
        )
        .bind(checklist_id)
        .bind(check_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound("check"))?;
        check_from_row(&row)
    }
}

fn checklist_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Checklist, StoreError> {
    Ok(Checklist {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
    })
}

fn check_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Check, StoreError> {
    Ok(Check {
        id: row.try_get("id")?,
        checklist_id: row.try_get("checklist_id")?,
        status: row.try_get("status")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        position: row.try_get("position")?,
    })
}
