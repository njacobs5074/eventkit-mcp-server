use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// User-configured default source for creating new reminder lists.
    /// When absent, the system default source is used instead.
    pub default_reminder_source_id: Option<String>,
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("eventkit-mcp-server")
        .join("config.json")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(data) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }
}
