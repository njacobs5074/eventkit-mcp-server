use std::sync::Arc;

use chrono::DateTime;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use highlandcows_eventkit::{EventKitError, FullAccessToken, Reminder, ReminderStore};

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ReminderListJson {
    id: String,
    title: String,
    allows_modifications: bool,
    source: Option<String>,
}

#[derive(Serialize)]
struct ReminderJson {
    id: Option<String>,
    title: String,
    notes: Option<String>,
    list_id: Option<String>,
    due_date: Option<String>,
    is_completed: bool,
    /// 0 = none, 1 = high, 5 = medium, 9 = low
    priority: u8,
}

// ── Input parameter types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListRemindersParams {
    /// Identifier of the Reminder list to filter by (omit for all lists)
    list_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReminderIdParams {
    /// Stable reminder identifier returned by list_reminders or create_reminder
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateReminderParams {
    /// Title of the reminder
    title: String,
    /// Optional notes body
    notes: Option<String>,
    /// Identifier of the Reminder list (omit to use the system default list)
    list_id: Option<String>,
    /// Due date in RFC 3339 format, e.g. "2026-07-01T09:00:00Z" (optional)
    due_date: Option<String>,
    /// Priority: 0 = none (default), 1 = high, 5 = medium, 9 = low
    priority: Option<u8>,
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct EventKitServer {
    store: ReminderStore,
    token: Arc<FullAccessToken>,
    #[allow(dead_code)]
    tool_router: ToolRouter<EventKitServer>,
}

impl Clone for EventKitServer {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            token: Arc::clone(&self.token),
            tool_router: Self::tool_router(),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn eventkit_err(e: EventKitError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn json_text(v: &impl Serialize) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(v).unwrap_or_else(|e| e.to_string()),
    )])
}

// ── Tool implementations ──────────────────────────────────────────────────────

#[tool_router]
impl EventKitServer {
    pub fn new(store: ReminderStore, token: FullAccessToken) -> Self {
        Self {
            store,
            token: Arc::new(token),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List all Reminder lists visible to the current user")]
    fn list_reminder_lists(&self) -> Result<CallToolResult, McpError> {
        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let lists = tokio::task::block_in_place(|| store.lists(&token)).map_err(eventkit_err)?;
        let out: Vec<ReminderListJson> = lists
            .iter()
            .map(|l| ReminderListJson {
                id: l.list_identifier.clone(),
                title: l.title.clone(),
                allows_modifications: l.allows_content_modifications,
                source: l.source_title.clone(),
            })
            .collect();
        Ok(json_text(&out))
    }

    #[tool(
        description = "List incomplete reminders, optionally filtered to a specific Reminder list"
    )]
    fn list_reminders(
        &self,
        Parameters(params): Parameters<ListRemindersParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let list_id = params.list_id;
        let reminders = tokio::task::block_in_place(|| {
            let ids: Option<Vec<&str>> = list_id.as_deref().map(|id| vec![id]);
            store.fetch_incomplete(ids.as_deref(), &token)
        })
        .map_err(eventkit_err)?;
        let out: Vec<ReminderJson> = reminders.iter().map(reminder_to_json).collect();
        Ok(json_text(&out))
    }

    #[tool(description = "Fetch a single reminder by its stable identifier")]
    fn get_reminder(
        &self,
        Parameters(params): Parameters<ReminderIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let id = params.id.clone();
        let reminder =
            tokio::task::block_in_place(|| store.fetch(&id, &token)).map_err(eventkit_err)?;
        match reminder {
            Some(r) => Ok(json_text(&reminder_to_json(&r))),
            None => Err(McpError::invalid_params(
                format!("reminder not found: {}", params.id),
                None,
            )),
        }
    }

    #[tool(
        description = "Create a new reminder. Returns the stable identifier of the created reminder."
    )]
    fn create_reminder(
        &self,
        Parameters(params): Parameters<CreateReminderParams>,
    ) -> Result<CallToolResult, McpError> {
        let due_date = params
            .due_date
            .as_deref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| McpError::invalid_params(format!("invalid due_date: {e}"), None))
            })
            .transpose()?;

        let reminder = Reminder {
            title: params.title,
            notes: params.notes,
            list_identifier: params.list_id,
            due_date,
            priority: params.priority.unwrap_or(0),
            ..Default::default()
        };

        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let id = tokio::task::block_in_place(|| store.save(&reminder, &*token))
            .map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(id)]))
    }

    #[tool(description = "Mark a reminder as completed")]
    fn complete_reminder(
        &self,
        Parameters(params): Parameters<ReminderIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let id = params.id.clone();
        tokio::task::block_in_place(|| store.complete(&id, &token)).map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "reminder {} marked as completed",
            params.id
        ))]))
    }

    #[tool(description = "Delete a reminder by its stable identifier")]
    fn delete_reminder(
        &self,
        Parameters(params): Parameters<ReminderIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.store.clone();
        let token = Arc::clone(&self.token);
        let id = params.id.clone();
        tokio::task::block_in_place(|| store.remove(&id, &token)).map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "reminder {} deleted",
            params.id
        ))]))
    }
}

#[tool_handler]
impl ServerHandler for EventKitServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Provides tools to read and manage macOS Reminders via Apple EventKit. \
                 Tools: list_reminder_lists, list_reminders, get_reminder, \
                 create_reminder, complete_reminder, delete_reminder.",
            )
    }
}

// ── Conversion ────────────────────────────────────────────────────────────────

fn reminder_to_json(r: &Reminder) -> ReminderJson {
    ReminderJson {
        id: r.identifier.clone(),
        title: r.title.clone(),
        notes: r.notes.clone(),
        list_id: r.list_identifier.clone(),
        due_date: r.due_date.map(|d| d.to_rfc3339()),
        is_completed: r.is_completed,
        priority: r.priority,
    }
}
