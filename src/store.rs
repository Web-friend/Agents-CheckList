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
    pub created_at: String,
    pub order: i64,
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
        let has_legacy_title: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('checklists') WHERE name = 'title'",
        )
        .fetch_one(&self.pool)
        .await?;
        let result = if has_legacy_title > 0 {
            sqlx::query("INSERT INTO checklists (name, title) VALUES (?, ?)")
                .bind(name)
                .bind(name)
                .execute(&self.pool)
                .await?
        } else {
            sqlx::query("INSERT INTO checklists (name) VALUES (?)")
                .bind(name)
                .execute(&self.pool)
                .await?
        };
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
        let result = sqlx::query(
            "INSERT INTO checks (checklist_id, title, description)
             VALUES (?, ?, ?)",
        )
        .bind(checklist_id)
        .bind(title)
        .bind(description)
        .execute(&self.pool)
        .await?;
        self.get_check(checklist_id, result.last_insert_rowid()).await
    }

    pub async fn list_checks(
        &self,
        checklist_id: i64,
        undone_only: bool,
    ) -> Result<Vec<Check>, StoreError> {
        let checks = self.get_order(checklist_id).await?;
        if undone_only {
            Ok(checks.into_iter().filter(|check| !check.status).collect())
        } else {
            Ok(checks)
        }
    }

    /// Returns checks in creation order, with a one-based order within the checklist.
    pub async fn get_order(&self, checklist_id: i64) -> Result<Vec<Check>, StoreError> {
        self.get_checklist(checklist_id).await?;
        let rows = sqlx::query(
            "SELECT id, checklist_id, status, title, description, created_at
             FROM checks WHERE checklist_id = ? ORDER BY created_at, id",
        )
        .bind(checklist_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .enumerate()
            .map(|(index, row)| check_from_row(row, index as i64 + 1))
            .collect()
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
        self.get_order(checklist_id)
            .await?
            .into_iter()
            .find(|check| check.id == check_id)
            .ok_or(StoreError::NotFound("check"))
    }
}

fn checklist_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Checklist, StoreError> {
    Ok(Checklist {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
    })
}

fn check_from_row(row: &sqlx::sqlite::SqliteRow, order: i64) -> Result<Check, StoreError> {
    Ok(Check {
        id: row.try_get("id")?,
        checklist_id: row.try_get("checklist_id")?,
        status: row.try_get("status")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        created_at: row.try_get("created_at")?,
        order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_with_url;
    use tempfile::TempDir;

    async fn test_store() -> (TempDir, Store) {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let database_path = directory.path().join("unit.sqlite");
        let database_url = format!("sqlite://{}", database_path.display());
        let pool = connect_with_url(&database_url).await.expect("open test database");
        (directory, Store::new(pool))
    }

    #[tokio::test]
    async fn checks_are_ordered_by_creation_and_status_can_be_changed() {
        let (_directory, store) = test_store().await;
        let checklist = store.create_checklist("Work").await.unwrap();
        let first = store
            .add_check(checklist.id, "First", "Long description")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = store
            .add_check(checklist.id, "Second", "")
            .await
            .unwrap();

        let ordered = store.get_order(checklist.id).await.unwrap();
        assert_eq!(ordered.iter().map(|check| check.id).collect::<Vec<_>>(), [first.id, second.id]);
        assert_eq!(ordered.iter().map(|check| check.order).collect::<Vec<_>>(), [1, 2]);
        assert!(!ordered[0].status);
        assert!(ordered[0].created_at <= ordered[1].created_at);

        store
            .set_check_status(checklist.id, first.id, true)
            .await
            .unwrap();
        let undone = store.list_checks(checklist.id, true).await.unwrap();
        assert_eq!(undone.len(), 1);
        assert_eq!(undone[0].id, second.id);
    }

    #[tokio::test]
    async fn deleting_checklist_cascades_to_checks() {
        let (_directory, store) = test_store().await;
        let checklist = store.create_checklist("Delete me").await.unwrap();
        store.add_check(checklist.id, "Child", "").await.unwrap();

        store.delete_checklist(checklist.id).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checks WHERE checklist_id = ?")
            .bind(checklist.id)
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn missing_checklist_is_reported() {
        let (_directory, store) = test_store().await;
        assert!(matches!(
            store.get_checklist(999).await,
            Err(StoreError::NotFound("checklist"))
        ));
    }
}
