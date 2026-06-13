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

use highlandcows_eventkit::{
    CalendarEvent, CalendarFullAccessToken, CalendarStore, EventKitError, FullAccessToken,
    Reminder, ReminderStore,
};

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

#[derive(Serialize)]
struct CalendarJson {
    id: String,
    title: String,
    allows_modifications: bool,
    source: Option<String>,
}

#[derive(Serialize)]
struct CalendarEventJson {
    id: Option<String>,
    title: String,
    notes: Option<String>,
    calendar_id: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    is_all_day: bool,
    location: Option<String>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListEventsParams {
    /// Start of the date range in RFC 3339 format, e.g. "2026-06-01T00:00:00Z"
    start: String,
    /// End of the date range in RFC 3339 format, e.g. "2026-06-30T23:59:59Z"
    end: String,
    /// Identifier of the Calendar to filter by (omit for all calendars)
    calendar_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EventIdParams {
    /// Stable event identifier returned by list_events or create_event
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateEventParams {
    /// Title of the event
    title: String,
    /// Start date/time in RFC 3339 format, e.g. "2026-07-01T09:00:00Z"
    start: String,
    /// End date/time in RFC 3339 format, e.g. "2026-07-01T10:00:00Z"
    end: String,
    /// Optional notes body
    notes: Option<String>,
    /// Identifier of the Calendar to add the event to (omit to use the default calendar)
    calendar_id: Option<String>,
    /// Optional location string
    location: Option<String>,
    /// Whether this is an all-day event (default: false)
    is_all_day: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateEventParams {
    /// Stable identifier of the event to update
    id: String,
    /// New title (omit to keep current)
    title: Option<String>,
    /// New start date/time in RFC 3339 format (omit to keep current)
    start: Option<String>,
    /// New end date/time in RFC 3339 format (omit to keep current)
    end: Option<String>,
    /// New notes body (omit to keep current)
    notes: Option<String>,
    /// New location (omit to keep current)
    location: Option<String>,
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct EventKitServer {
    reminder_store: ReminderStore,
    reminder_token: Arc<FullAccessToken>,
    calendar_store: CalendarStore,
    calendar_token: Arc<CalendarFullAccessToken>,
    #[allow(dead_code)]
    tool_router: ToolRouter<EventKitServer>,
}

impl Clone for EventKitServer {
    fn clone(&self) -> Self {
        Self {
            reminder_store: self.reminder_store.clone(),
            reminder_token: Arc::clone(&self.reminder_token),
            calendar_store: self.calendar_store.clone(),
            calendar_token: Arc::clone(&self.calendar_token),
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

fn parse_datetime(s: &str, field: &str) -> Result<DateTime<chrono::Utc>, McpError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| McpError::invalid_params(format!("invalid {field}: {e}"), None))
}

// ── Tool implementations ──────────────────────────────────────────────────────

#[tool_router]
impl EventKitServer {
    pub fn new(
        reminder_store: ReminderStore,
        reminder_token: FullAccessToken,
        calendar_store: CalendarStore,
        calendar_token: CalendarFullAccessToken,
    ) -> Self {
        Self {
            reminder_store,
            reminder_token: Arc::new(reminder_token),
            calendar_store,
            calendar_token: Arc::new(calendar_token),
            tool_router: Self::tool_router(),
        }
    }

    // ── Reminders ─────────────────────────────────────────────────────────────

    #[tool(description = "List all Reminder lists visible to the current user")]
    fn list_reminder_lists(&self) -> Result<CallToolResult, McpError> {
        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
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
        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
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
        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
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
            .map(|s| parse_datetime(s, "due_date"))
            .transpose()?;

        let reminder = Reminder {
            title: params.title,
            notes: params.notes,
            list_identifier: params.list_id,
            due_date,
            priority: params.priority.unwrap_or(0),
            ..Default::default()
        };

        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
        let id = tokio::task::block_in_place(|| store.save(&reminder, &*token))
            .map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(id)]))
    }

    #[tool(description = "Mark a reminder as completed")]
    fn complete_reminder(
        &self,
        Parameters(params): Parameters<ReminderIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
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
        let store = self.reminder_store.clone();
        let token = Arc::clone(&self.reminder_token);
        let id = params.id.clone();
        tokio::task::block_in_place(|| store.remove(&id, &token)).map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "reminder {} deleted",
            params.id
        ))]))
    }

    // ── Calendar ──────────────────────────────────────────────────────────────

    #[tool(description = "List all Calendars visible to the current user")]
    fn list_calendars(&self) -> Result<CallToolResult, McpError> {
        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let calendars =
            tokio::task::block_in_place(|| store.lists(&token)).map_err(eventkit_err)?;
        let out: Vec<CalendarJson> = calendars
            .iter()
            .map(|c| CalendarJson {
                id: c.calendar_identifier.clone(),
                title: c.title.clone(),
                allows_modifications: c.allows_content_modifications,
                source: c.source_title.clone(),
            })
            .collect();
        Ok(json_text(&out))
    }

    #[tool(
        description = "List calendar events in a date range, optionally filtered to a specific Calendar"
    )]
    fn list_events(
        &self,
        Parameters(params): Parameters<ListEventsParams>,
    ) -> Result<CallToolResult, McpError> {
        let start = parse_datetime(&params.start, "start")?;
        let end = parse_datetime(&params.end, "end")?;
        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let calendar_id = params.calendar_id;
        let events = tokio::task::block_in_place(|| {
            let ids: Option<Vec<&str>> = calendar_id.as_deref().map(|id| vec![id]);
            store.fetch_in_range(start, end, ids.as_deref(), &token)
        })
        .map_err(eventkit_err)?;
        let out: Vec<CalendarEventJson> = events.iter().map(event_to_json).collect();
        Ok(json_text(&out))
    }

    #[tool(description = "Fetch a single calendar event by its stable identifier")]
    fn get_event(
        &self,
        Parameters(params): Parameters<EventIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let id = params.id.clone();
        let event =
            tokio::task::block_in_place(|| store.fetch(&id, &token)).map_err(eventkit_err)?;
        match event {
            Some(e) => Ok(json_text(&event_to_json(&e))),
            None => Err(McpError::invalid_params(
                format!("event not found: {}", params.id),
                None,
            )),
        }
    }

    #[tool(
        description = "Create a new calendar event. Returns the stable identifier of the created event."
    )]
    fn create_event(
        &self,
        Parameters(params): Parameters<CreateEventParams>,
    ) -> Result<CallToolResult, McpError> {
        let start = parse_datetime(&params.start, "start")?;
        let end = parse_datetime(&params.end, "end")?;

        let event = CalendarEvent {
            title: params.title,
            notes: params.notes,
            calendar_identifier: params.calendar_id,
            start_date: Some(start),
            end_date: Some(end),
            location: params.location,
            is_all_day: params.is_all_day.unwrap_or(false),
            ..Default::default()
        };

        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let id = tokio::task::block_in_place(|| store.save(&event, &*token))
            .map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(id)]))
    }

    #[tool(
        description = "Update an existing calendar event. Only the fields you supply are changed."
    )]
    fn update_event(
        &self,
        Parameters(params): Parameters<UpdateEventParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let id = params.id.clone();

        // Fetch current state.
        let mut event = tokio::task::block_in_place(|| store.fetch(&id, &token))
            .map_err(eventkit_err)?
            .ok_or_else(|| McpError::invalid_params(format!("event not found: {id}"), None))?;

        // Apply supplied fields.
        if let Some(title) = params.title {
            event.title = title;
        }
        if let Some(s) = params.start {
            event.start_date = Some(parse_datetime(&s, "start")?);
        }
        if let Some(e) = params.end {
            event.end_date = Some(parse_datetime(&e, "end")?);
        }
        if let Some(notes) = params.notes {
            event.notes = Some(notes);
        }
        if let Some(location) = params.location {
            event.location = Some(location);
        }

        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        tokio::task::block_in_place(|| store.save(&event, &*token)).map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "event {id} updated"
        ))]))
    }

    #[tool(description = "Delete a calendar event by its stable identifier")]
    fn delete_event(
        &self,
        Parameters(params): Parameters<EventIdParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.calendar_store.clone();
        let token = Arc::clone(&self.calendar_token);
        let id = params.id.clone();
        tokio::task::block_in_place(|| store.remove(&id, &*token)).map_err(eventkit_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "event {} deleted",
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
                "Provides tools to read and manage macOS Reminders and Calendar events \
                 via Apple EventKit.\n\
                 Reminders tools: list_reminder_lists, list_reminders, get_reminder, \
                 create_reminder, complete_reminder, delete_reminder.\n\
                 Calendar tools: list_calendars, list_events, get_event, \
                 create_event, update_event, delete_event.",
            )
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

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

fn event_to_json(e: &CalendarEvent) -> CalendarEventJson {
    CalendarEventJson {
        id: e.identifier.clone(),
        title: e.title.clone(),
        notes: e.notes.clone(),
        calendar_id: e.calendar_identifier.clone(),
        start_date: e.start_date.map(|d| d.to_rfc3339()),
        end_date: e.end_date.map(|d| d.to_rfc3339()),
        is_all_day: e.is_all_day,
        location: e.location.clone(),
    }
}
