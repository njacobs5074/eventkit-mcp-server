# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## Overview

`eventkit-mcp-server` is a local MCP server (stdio transport) that exposes macOS
Reminders and Calendar events via Apple's EventKit framework. It depends on
`highlandcows-eventkit` from the sibling `highlandcows/` project.

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

There are no automated tests — the server requires a live macOS Reminders and
Calendar database with TCC authorization. Test manually by running the binary and
connecting an MCP client, or use the MCP Inspector:

```bash
npx @modelcontextprotocol/inspector target/debug/eventkit-mcp-server
```

## Architecture

```
src/
  main.rs      — entry point: connect, authorize both stores, serve on stdio, wait
  server.rs    — EventKitServer: tool definitions + ServerHandler impl
```

### Key design decisions

- **Startup authorization**: Both Reminders and Calendar authorization happen in
  `main()` before the MCP server starts. If either fails the process exits
  immediately with a clear error. This avoids partial initialization and ensures
  every tool call has a valid token.

- **Tokens in `Arc`**: `FullAccessToken` and `CalendarFullAccessToken` from
  `highlandcows-eventkit` are not `Clone` (intentional — they are compile-time
  capability tokens). Both are stored in `Arc<T>` so `EventKitServer` can implement
  `Clone` (required by `#[tool_router]`) without duplicating the tokens.

- **Sync EventKit calls via `block_in_place`**: EventKit's APIs are synchronous.
  Tool handlers call `tokio::task::block_in_place` to safely block a tokio worker
  thread without starving the runtime. This works because `#[tokio::main]` uses the
  multi-threaded scheduler.

- **`&*token` for `save` and `remove`**: `ReminderStore::save`,
  `CalendarStore::save`, and `CalendarStore::remove` take `&impl <Trait>` (generic
  bound), so auto-deref coercions from `Arc<T>` don't apply. Those call sites use
  `&*token` to explicitly deref through the `Arc`. Other methods take the concrete
  token type directly and accept `&token`.

- **`update_event` fetch-then-save**: The update tool fetches the current event,
  patches only the fields the caller supplied, then saves. This avoids callers
  having to re-supply unchanged fields and prevents accidental data loss.

### Dependency relationship

```
eventkit-mcp-server
  └── highlandcows-eventkit (local path patch → ../highlandcows/crates/eventkit)
        └── objc2-event-kit (Apple EventKit Objective-C bindings)
```

The `[patch.crates-io]` override in `Cargo.toml` redirects the declared
`"0.4.0"` dependency to the local crate. This is necessary because the Calendar
API was added after `0.4.0` was published. Remove the patch and bump the version
once a new version is published.

## Adding new tools

1. Add a parameter struct (if needed) deriving `Deserialize` and `schemars::JsonSchema`
2. Add the tool method to the `#[tool_router]` impl block with a `#[tool(description = "...")]` attribute
3. The method signature is `fn tool_name(&self, Parameters(p): Parameters<MyParams>) -> Result<CallToolResult, McpError>`
4. Use `tokio::task::block_in_place(|| ...)` to call synchronous EventKit methods
5. Update the `get_info()` instructions string in the `ServerHandler` impl
