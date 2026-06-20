#[cfg(not(target_os = "macos"))]
compile_error!("eventkit-mcp-server only supports macOS");

mod config;
mod server;

use anyhow::Result;
use highlandcows_eventkit::{CalendarStore, ReminderStore};
use rmcp::{ServiceExt, transport::stdio};
use server::EventKitServer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let reminder_store = ReminderStore::builder()
        .connect()
        .map_err(|e| anyhow::anyhow!("failed to connect to Reminders: {e}"))?;
    let reminder_token = reminder_store
        .authorize()
        .map_err(|e| anyhow::anyhow!("Reminders authorization failed: {e}"))?;
    tracing::info!("Reminders authorized");

    let calendar_store = CalendarStore::builder()
        .connect()
        .map_err(|e| anyhow::anyhow!("failed to connect to Calendar: {e}"))?;
    let calendar_token = calendar_store
        .authorize()
        .map_err(|e| anyhow::anyhow!("Calendar authorization failed: {e}"))?;
    tracing::info!("Calendar authorized — starting MCP server on stdio");

    let config = config::Config::load();

    // Resolve the system default source once at startup so it can be
    // surfaced in get_info() and used as the fallback for create_reminder_list.
    let system_default_source = reminder_store
        .default_source(&reminder_token)
        .unwrap_or(None);

    let service = EventKitServer::new(
        reminder_store,
        reminder_token,
        calendar_store,
        calendar_token,
        config,
        system_default_source,
    )
    .serve(stdio())
    .await
    .inspect_err(|e| tracing::error!("server error: {e:?}"))?;

    service.waiting().await?;
    Ok(())
}
