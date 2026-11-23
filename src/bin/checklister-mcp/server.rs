use std::sync::Arc;

use checklister::Store;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChecklistIdParams {
    #[serde(deserialize_with = "deserialize_positive_id")]
    #[schemars(
        range(min = 1),
        description = "ID of the checklist. Use create_checklist or list_checklists to obtain it."
    )]
    pub checklist_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckIdParams {
    #[serde(deserialize_with = "deserialize_positive_id")]
    #[schemars(
        range(min = 1),
        description = "ID of the checklist. Use create_checklist or list_checklists to obtain it."
    )]
    pub checklist_id: i64,
    #[serde(deserialize_with = "deserialize_positive_id")]
    #[schemars(
        range(min = 1),
        description = "ID of the check within the checklist. Use add_check, list_checks, or show_checklist to obtain it."
    )]
    pub check_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateChecklistParams {
    #[serde(deserialize_with = "deserialize_non_empty_string")]
    #[schemars(
        length(min = 1),
        description = "Display name for the new checklist. Must be non-empty."
    )]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddCheckParams {
    #[serde(deserialize_with = "deserialize_positive_id")]
    #[schemars(
        range(min = 1),
        description = "ID of the checklist. Use create_checklist or list_checklists to obtain it."
    )]
    pub checklist_id: i64,
    #[serde(deserialize_with = "deserialize_non_empty_string")]
    #[schemars(
        length(min = 1),
        description = "Short title for the new check. Must be non-empty."
    )]
    pub title: String,
    #[schemars(
        description = "Detailed description for the new check. Use an empty string when there is none."
    )]
    pub description: String,
}

fn deserialize_positive_id<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = i64::deserialize(deserializer)?;
    if value <= 0 {
        return Err(serde::de::Error::custom("ID must be a positive integer"));
    }
    Ok(value)
}

fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(serde::de::Error::custom("value must not be empty"));
    }
    Ok(value)
}

#[derive(Serialize)]
struct Mutation<T> {
    success: bool,
    message: &'static str,
    result: T,
}

fn text_result(value: &impl Serialize) -> CallToolResult {
    match serde_json::to_string_pretty(value) {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => tool_error(format!("failed to serialize response: {error}")),
    }
}

fn tool_error(error: impl std::fmt::Display) -> CallToolResult {
    #[derive(Serialize)]
    struct ErrorPayload {
        success: bool,
        error: String,
    }

    let payload = ErrorPayload {
        success: false,
        error: error.to_string(),
    };
    let text = serde_json::to_string(&payload)
        .unwrap_or_else(|_| "{\"success\":false,\"error\":\"serialization failed\"}".to_owned());
    CallToolResult::error(vec![ContentBlock::text(text)])
}

#[derive(Clone)]
pub struct ChecklisterServer {
    store: Arc<Store>,
    tool_router: ToolRouter<Self>,
}

impl ChecklisterServer {
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ChecklisterServer {
    #[tool(
        description = "Create a new, empty checklist. Use this first when you need a container for checks; the response contains the generated id required by every checklist-scoped tool. Returns {success, message, result: {id, name}}.",
        title = "Create checklist",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_checklist(
        &self,
        Parameters(params): Parameters<CreateChecklistParams>,
    ) -> CallToolResult {
        match self.store.create_checklist(&params.name).await {
            Ok(checklist) => text_result(&Mutation {
                success: true,
                message: "Checklist created",
                result: checklist,
            }),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "List every checklist, ordered by id. Use this to discover checklist ids before calling checklist-scoped tools. Returns an array of {id, name}; an empty array means no checklists exist.",
        title = "List checklists",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_checklists(&self) -> CallToolResult {
        match self.store.list_checklists().await {
            Ok(checklists) => text_result(&checklists),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Show one checklist with all of its checks, completed and undone, in creation order. Each check includes id, checklist_id, status, title, description, created_at, and a 1-based order. Returns {checklist, checks}; fails if the checklist does not exist.",
        title = "Show checklist",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn show_checklist(
        &self,
        Parameters(params): Parameters<ChecklistIdParams>,
    ) -> CallToolResult {
        match self.store.show_checklist(params.checklist_id).await {
            Ok(details) => text_result(&details),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Permanently delete a checklist and all of its checks. This cannot be undone. Returns the deleted checklist; fails if it does not exist. Prefer confirming with the user before calling when deletion is not clearly intended.",
        title = "Delete checklist",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_checklist(
        &self,
        Parameters(params): Parameters<ChecklistIdParams>,
    ) -> CallToolResult {
        match self.store.delete_checklist(params.checklist_id).await {
            Ok(checklist) => text_result(&Mutation {
                success: true,
                message: "Checklist deleted",
                result: checklist,
            }),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Add a new check to an existing checklist. New checks start undone (status false) and are appended in creation order. title must be non-empty; description may be an empty string. Returns the created check; fails if the checklist does not exist.",
        title = "Add check",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_check(&self, Parameters(params): Parameters<AddCheckParams>) -> CallToolResult {
        match self
            .store
            .add_check(params.checklist_id, &params.title, &params.description)
            .await
        {
            Ok(check) => text_result(&Mutation {
                success: true,
                message: "Check added",
                result: check,
            }),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "List all checks in a checklist, complete and undone, in creation order. Returns an array of check objects with id, checklist_id, status, title, description, created_at, and 1-based order, or an empty array. Fails if the checklist does not exist.",
        title = "List checks",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_checks(
        &self,
        Parameters(params): Parameters<ChecklistIdParams>,
    ) -> CallToolResult {
        match self.store.list_checks(params.checklist_id, false).await {
            Ok(checks) => text_result(&checks),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "List only the still-undone checks (status false) in a checklist, in creation order. Use this to find remaining work or the next check to complete. Fails if the checklist does not exist.",
        title = "List undone checks",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_undone_checks(
        &self,
        Parameters(params): Parameters<ChecklistIdParams>,
    ) -> CallToolResult {
        match self.store.list_checks(params.checklist_id, true).await {
            Ok(checks) => text_result(&checks),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Mark a check complete (status true). The check must belong to checklist_id; completing an already-complete check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
        title = "Complete check",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn complete_check(
        &self,
        Parameters(params): Parameters<CheckIdParams>,
    ) -> CallToolResult {
        match self
            .store
            .set_check_status(params.checklist_id, params.check_id, true)
            .await
        {
            Ok(check) => text_result(&Mutation {
                success: true,
                message: "Check completed",
                result: check,
            }),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Reopen a completed check by marking it undone (status false). The check must belong to checklist_id; undoing an already-undone check succeeds without changing it. Returns the updated check; fails if the checklist or check does not exist.",
        title = "Undo check",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn undo_check(&self, Parameters(params): Parameters<CheckIdParams>) -> CallToolResult {
        match self
            .store
            .set_check_status(params.checklist_id, params.check_id, false)
            .await
        {
            Ok(check) => text_result(&Mutation {
                success: true,
                message: "Check marked undone",
                result: check,
            }),
            Err(error) => tool_error(error),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ChecklisterServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new("checklister", "0.1.0"))
    }
}
