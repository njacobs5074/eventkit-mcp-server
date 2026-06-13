# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## Overview

`eventkit-mcp-server` is a local MCP server (stdio transport) that exposes macOS
Reminders via Apple's EventKit framework. It depends on the published
`highlandcows-eventkit` crate from the sibling `highlandcows/` project.

## Workflow

For any non-trivial change, create a branch before editing.

Branch naming follows conventional-commit style:

| Type | Example branch |
|------|----------------|
| Feature | `feat/calendar-tools` |
| Bug fix | `fix/auth-retry` |
| Refactor | `refactor/server-split` |
| Docs | `docs/readme-config` |
| Chore | `chore/update-dependencies` |

## Commands

```bash
cargo build            # build dev binary
cargo build --release  # build release binary
cargo clippy           # lint
cargo fmt              # format
```

There are no automated tests — the server requires a live macOS Reminders database
and TCC authorization. Test manually by running the binary and connecting an MCP
client, or use the MCP Inspector:

```bash
npx @modelcontextprotocol/inspector target/debug/eventkit-mcp-server
```

## Architecture

```
src/
  main.rs      — entry point: connect, authorize, serve on stdio, wait
  server.rs    — EventKitServer: tool definitions + ServerHandler impl
```

### Key design decisions

- **Startup authorization**: Reminders authorization happens in `main()` before the
  MCP server starts. If authorization fails the process exits immediately with a
  clear error. This avoids partial initialization and ensures every tool call has a
  valid token.

- **`FullAccessToken` in `Arc`**: `FullAccessToken` from `highlandcows-eventkit` is
  not `Clone` (intentional — it's a compile-time capability token). It's stored in
  `Arc<FullAccessToken>` so the `EventKitServer` can implement `Clone` (required by
  `#[tool_router]`) without duplicating the token.

- **Sync EventKit calls via `block_in_place`**: EventKit's Reminders APIs are
  synchronous. Tool handlers call `tokio::task::block_in_place` to safely block a
  tokio worker thread without starving the runtime. This works because
  `#[tokio::main]` uses the multi-threaded scheduler.

- **`&*token` vs `&token` for `save`**: `ReminderStore::save` takes
  `&impl RemindersAccess`, which requires the concrete type to be resolved.
  Auto-deref coercions don't apply here, so the explicit dereference `&*token`
  is needed to go from `&Arc<FullAccessToken>` → `&FullAccessToken`. Other methods
  take `&FullAccessToken` directly and can use `&token` with auto-deref.

### Dependency relationship

```
eventkit-mcp-server
  └── highlandcows-eventkit (crates.io, "0.4.0")
        └── objc2-event-kit (Apple EventKit Objective-C bindings)
```

To develop against local, unpublished changes to `highlandcows-eventkit`, add a
patch override in `Cargo.toml`:

```toml
[patch.crates-io]
highlandcows-eventkit = { path = "../highlandcows/crates/eventkit" }
```

Remove the patch before committing.

## Adding new tools

1. Add a parameter struct (if needed) deriving `Deserialize` and `schemars::JsonSchema`
2. Add the tool method to the `#[tool_router]` impl block with a `#[tool(description = "...")]` attribute
3. The method signature is `fn tool_name(&self, Parameters(p): Parameters<MyParams>) -> Result<CallToolResult, McpError>`
4. Use `tokio::task::block_in_place(|| ...)` to call synchronous EventKit methods
5. Update `get_info()` instructions string in the `ServerHandler` impl

## Roadmap

- Calendar event tools (blocked on `CalendarStore` save/remove in `highlandcows-eventkit`)
