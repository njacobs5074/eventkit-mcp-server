# eventkit-mcp-server

A local [Model Context Protocol](https://modelcontextprotocol.io) server that exposes macOS Reminders and Calendar events via Apple's EventKit framework.

> **Created with [Claude Code](https://claude.ai/code) by Anthropic.**

---

## Requirements

- macOS (EventKit is Apple-only)
- Reminders access granted to your terminal in **System Settings → Privacy & Security → Reminders**
- Calendar access granted to your terminal in **System Settings → Privacy & Security → Calendar**

## Building

```bash
cargo build --release
```

The binary is written to `target/release/eventkit-mcp-server`.

## Usage

The server uses stdio transport. On startup it connects to the system Reminders and Calendar databases and requests authorization — on first run macOS shows a permission dialog naming your terminal. Once authorized it serves MCP requests over stdin/stdout until the client disconnects.

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

### Reminders

| Tool | Description |
|------|-------------|
| `list_reminder_lists` | List all Reminder lists visible to the current user |
| `list_reminders` | List incomplete reminders, optionally filtered by list |
| `get_reminder` | Fetch a single reminder by its stable identifier |
| `create_reminder` | Create a new reminder |
| `complete_reminder` | Mark a reminder as completed |
| `delete_reminder` | Delete a reminder by its stable identifier |

### Calendar

| Tool | Description |
|------|-------------|
| `list_calendars` | List all Calendars visible to the current user |
| `list_events` | List events in a date range, optionally filtered by calendar |
| `get_event` | Fetch a single event by its stable identifier |
| `create_event` | Create a new calendar event |
| `update_event` | Update fields on an existing event |
| `delete_event` | Delete an event by its stable identifier |

---

## Tool inputs

### Reminders

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

### Calendar

**`list_events`**
```json
{
  "start": "2026-06-01T00:00:00Z",
  "end": "2026-06-30T23:59:59Z",
  "calendar_id": "optional-calendar-identifier"
}
```

**`get_event`** / **`delete_event`**
```json
{ "id": "event-identifier" }
```

**`create_event`**
```json
{
  "title": "Team sync",
  "start": "2026-07-01T09:00:00Z",
  "end": "2026-07-01T10:00:00Z",
  "notes": "Quarterly review",
  "calendar_id": "optional-calendar-identifier",
  "location": "Conf room A",
  "is_all_day": false
}
```

**`update_event`**
```json
{
  "id": "event-identifier",
  "title": "Updated title",
  "start": "2026-07-01T10:00:00Z",
  "end": "2026-07-01T11:00:00Z",
  "notes": "New notes",
  "location": "Conf room B"
}
```
All fields except `id` are optional — only the fields you supply are changed.

---

## Known limitations

**Requires a stdio-capable MCP client.** This server uses stdio transport, which means it must be launched as a subprocess by the MCP client. It works with Claude Desktop and Claude Code but is not compatible with remote or HTTP-based MCP clients.

**Reminders tags are not supported.** Apple does not expose tag read or write access through the public EventKit API, so this server cannot read, set, or filter by native Reminders tags. Tags you have assigned in the Reminders app will not appear in tool output, and `create_reminder` / `update_reminder` have no tag parameter. This is a limitation of EventKit itself, not this server.

---

## Tracing

The server logs to stderr (never stdout — stdout is reserved for MCP JSON-RPC framing). Verbosity is controlled by the `RUST_LOG` environment variable using the standard `tracing` filter syntax:

| Level | What you see |
|-------|-------------|
| `error` | Only fatal errors |
| `warn` | Errors and warnings (recommended for normal use) |
| `info` | Startup confirmation and per-request summaries (default) |
| `debug` | Detailed request/response tracing |

### Claude Desktop

Add an `env` block to the server entry in `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "eventkit": {
      "command": "/path/to/eventkit-mcp-server",
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```

### Claude Code

Add an `env` block to the server entry in `~/.claude.json` (global) or your project's `.claude/settings.json`:

```json
{
  "mcpServers": {
    "eventkit": {
      "type": "stdio",
      "command": "/path/to/eventkit-mcp-server",
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```

Logs are written to:

- **Claude Desktop:** `~/Library/Logs/Claude/mcp-server-eventkit.log`
- **Claude Code:** `~/Library/Logs/Claude/mcp-server-eventkit.log`
