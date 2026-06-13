#[cfg(not(target_os = "macos"))]
compile_error!("eventkit-mcp-server only supports macOS");

mod server;

use anyhow::Result;
use highlandcows_eventkit::ReminderStore;
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

    let store = ReminderStore::builder()
        .connect()
        .map_err(|e| anyhow::anyhow!("failed to connect to Reminders: {e}"))?;

    let token = store
        .authorize()
        .map_err(|e| anyhow::anyhow!("Reminders authorization failed: {e}"))?;

    tracing::info!("Reminders authorized — starting MCP server on stdio");

    let service = EventKitServer::new(store, token)
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("server error: {e:?}"))?;

    service.waiting().await?;
    Ok(())
}
