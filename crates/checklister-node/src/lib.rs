use checklister::{connect_with_url, Store};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;

fn to_napi(error: impl std::fmt::Display) -> Error {
    Error::from_reason(error.to_string())
}

/// In-process bridge to the Checklister store, exposed to Node through Node-API.
#[napi]
pub struct Database {
    store: Store,
}

#[napi]
impl Database {
    /// Opens (or creates) the SQLite database, falling back to `DATABASE_URL`.
    #[napi(factory)]
    pub async fn open(database_url: Option<String>) -> Result<Self> {
        let url = database_url.unwrap_or_else(|| {
            std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://checklister.db".to_owned())
        });
        let pool = connect_with_url(&url).await.map_err(to_napi)?;
        Ok(Self {
            store: Store::new(pool),
        })
    }

    #[napi]
    pub async fn create_checklist(&self, name: String) -> Result<Value> {
        let checklist = self.store.create_checklist(&name).await.map_err(to_napi)?;
        serde_json::to_value(checklist).map_err(to_napi)
    }

    #[napi]
    pub async fn list_checklists(&self) -> Result<Value> {
        let checklists = self.store.list_checklists().await.map_err(to_napi)?;
        serde_json::to_value(checklists).map_err(to_napi)
    }

    #[napi]
    pub async fn show_checklist(&self, checklist_id: i64) -> Result<Value> {
        let details = self
            .store
            .show_checklist(checklist_id)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(details).map_err(to_napi)
    }

    #[napi]
    pub async fn delete_checklist(&self, checklist_id: i64) -> Result<Value> {
        let checklist = self
            .store
            .delete_checklist(checklist_id)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(checklist).map_err(to_napi)
    }

    #[napi]
    pub async fn add_check(
        &self,
        checklist_id: i64,
        title: String,
        description: String,
    ) -> Result<Value> {
        let check = self
            .store
            .add_check(checklist_id, &title, &description)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(check).map_err(to_napi)
    }

    #[napi]
    pub async fn list_checks(&self, checklist_id: i64, undone_only: bool) -> Result<Value> {
        let checks = self
            .store
            .list_checks(checklist_id, undone_only)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(checks).map_err(to_napi)
    }

    #[napi]
    pub async fn complete_check(&self, checklist_id: i64, check_id: i64) -> Result<Value> {
        let check = self
            .store
            .set_check_status(checklist_id, check_id, true)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(check).map_err(to_napi)
    }

    #[napi]
    pub async fn undo_check(&self, checklist_id: i64, check_id: i64) -> Result<Value> {
        let check = self
            .store
            .set_check_status(checklist_id, check_id, false)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(check).map_err(to_napi)
    }
}
