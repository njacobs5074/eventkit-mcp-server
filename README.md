# eventkit-mcp-server

A local [Model Context Protocol](https://modelcontextprotocol.io) server that exposes macOS Reminders via Apple's EventKit framework.

> **Created with [Claude Code](https://claude.ai/code) by Anthropic.**

---

## Requirements

- macOS (EventKit is Apple-only)
- Reminders access granted to your terminal in **System Settings → Privacy & Security → Reminders**

## Building

```bash
cargo build --release
```

The binary is written to `target/release/eventkit-mcp-server`.

## Usage

The server uses stdio transport. On startup it connects to the system Reminders database and requests authorization — on first run macOS shows a permission dialog naming your terminal. Once authorized it serves MCP requests over stdin/stdout until the client disconnects.

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "eventkit": {
      "command": "/path/to/eventkit-mcp-server/target/release/eventkit-mcp-server"
    }
  }
}
```

### Claude Code

Add to your project or global `.claude/settings.json`:

```json
{
  "mcpServers": {
    "eventkit": {
      "command": "/path/to/eventkit-mcp-server/target/release/eventkit-mcp-server",
      "type": "stdio"
    }
  }
}
```

---

## Tools

| Tool | Description |
|------|-------------|
| `list_reminder_lists` | List all Reminder lists visible to the current user |
| `list_reminders` | List incomplete reminders, optionally filtered by list |
| `get_reminder` | Fetch a single reminder by its stable identifier |
| `create_reminder` | Create a new reminder |
| `complete_reminder` | Mark a reminder as completed |
| `delete_reminder` | Delete a reminder by its stable identifier |

### Tool inputs

**`list_reminders`**
```json
{ "list_id": "optional-list-identifier" }
```

**`get_reminder`** / **`complete_reminder`** / **`delete_reminder`**
```json
{ "id": "reminder-identifier" }
```

**`create_reminder`**
```json
{
  "title": "Buy oat milk",
  "notes": "From the co-op",
  "list_id": "optional-list-identifier",
  "due_date": "2026-07-01T09:00:00Z",
  "priority": 0
}
```
`priority`: `0` = none (default), `1` = high, `5` = medium, `9` = low.  
`due_date`: RFC 3339 string (optional).

---

## Tracing

The server logs to stderr. Control verbosity with the `RUST_LOG` environment variable:

```bash
RUST_LOG=info eventkit-mcp-server
RUST_LOG=debug eventkit-mcp-server
```

---

## Roadmap

- [ ] Calendar events (read + write) via `highlandcows-eventkit` CalendarStore
